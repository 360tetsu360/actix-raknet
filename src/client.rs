use std::{net::SocketAddr, time::Duration};

use actix::{dev::ToEnvelope, io::SinkWrite, prelude::*};
use bytes::BytesMut;
use futures::StreamExt;
use tokio_util::{codec::BytesCodec, udp::UdpFramed};

use crate::{
    macros::unwrap_or_return,
    packets::*,
    session::{time, ReceivedDatagram, Session, SessionEnd},
    udp::{ReceivedUdp, SendUdp, UdpActor, UdpPacket},
    RAKNET_PROTOCOL_VERSION,
};

#[derive(Message)]
#[rtype(result = "()")]
enum RakClientMsg {
    Connect(SocketAddr),
    Packet(BytesMut),
    Disconnect,
}

pub struct ClientHandle {
    addr: Recipient<RakClientMsg>,
}

impl ClientHandle {
    pub fn connect(&self, address: SocketAddr) {
        unwrap_or_return!(self.addr.do_send(RakClientMsg::Connect(address)));
    }
    pub fn packet(&self, bytes: BytesMut) {
        unwrap_or_return!(self.addr.do_send(RakClientMsg::Packet(bytes)));
    }
    pub fn disconnect(&self) {
        unwrap_or_return!(self.addr.do_send(RakClientMsg::Disconnect));
    }
}

pub enum ConnectionFailedReason {
    AlreadyConnected,
    DifferentVersion,
    Timeout,
}
#[derive(Message)]
#[rtype(result = "()")]
pub enum RakClientEvent {
    ConnectionFailed(ConnectionFailedReason),
    Connected,
    Packet(BytesMut),
    Disconnected,
}

pub struct RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    udp: Addr<UdpActor<Self>>,
    guid: u64,
    mediator: Option<Addr<ClientMediator>>,
    handler: Addr<T>,
    session: Option<Session<Self>>,

    tick_handle: Option<SpawnHandle>,
    remote: Option<SocketAddr>,
    disconnect_handle: Option<SpawnHandle>,
}

impl<T> RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    pub fn init(socket: tokio::net::UdpSocket, guid: u64, handler: Addr<T>) -> ClientHandle {
        let (sink, stream) = UdpFramed::new(socket, BytesCodec::new()).split();
        let addr = Self::create(|ctx| Self {
            udp: UdpActor::create(|ctx2| {
                ctx2.add_stream(stream.filter_map(
                    |item: std::io::Result<(BytesMut, SocketAddr)>| async {
                        item.map(|(data, sender)| UdpPacket {
                            bytes: data,
                            addr: sender,
                        })
                        .ok()
                    },
                ));
                UdpActor {
                    sink: SinkWrite::new(sink, ctx2),
                    addr: ctx.address(),
                }
            }),
            guid,
            mediator: None,
            handler,
            session: None,
            tick_handle: None,
            remote: None,
            disconnect_handle: None,
        });
        ClientHandle {
            addr: addr.recipient::<RakClientMsg>(),
        }
    }
}

impl<T> RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    fn update(&mut self, ctx: &mut Context<Self>) {
        if self.session.is_some() {
            self.session.as_mut().unwrap().update();
        } else {
            ctx.cancel_future(self.tick_handle.unwrap());
        }
    }
    fn connection_timeout(&mut self) {
        self.handler.do_send(RakClientEvent::ConnectionFailed(
            ConnectionFailedReason::Timeout,
        ));
    }
}

impl<T> Actor for RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    type Context = Context<Self>;
}

impl<T> Handler<ReceivedUdp> for RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    type Result = ();
    fn handle(&mut self, msg: ReceivedUdp, _ctx: &mut Self::Context) -> Self::Result {
        if self.remote.is_none() {
            return;
        }

        if self.remote.unwrap() != msg.0.addr {
            return;
        }

        if let Some(mediator) = &self.mediator {
            mediator.do_send(msg);
        } else if self.session.is_some() {
            self.session.as_mut().unwrap().handle(msg);
        }
    }
}

impl<T> Handler<RakClientMsg> for RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    type Result = ();
    fn handle(&mut self, msg: RakClientMsg, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakClientMsg::Connect(addr) => {
                let udp = self.udp.clone().recipient::<SendUdp>();
                self.remote = Some(addr);
                self.mediator = Some(ClientMediator::new(
                    udp,
                    ctx.address().recipient::<MediatorEvent>(),
                    self.guid,
                    addr,
                ))
            }
            RakClientMsg::Packet(bytes) => {
                if self.session.is_some() {
                    self.session.as_mut().unwrap().send_to(bytes);
                }
            }
            RakClientMsg::Disconnect => {
                if self.session.is_some() {
                    self.session.as_mut().unwrap().disconnect();
                }
            }
        }
    }
}

impl<T> Handler<MediatorEvent> for RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    type Result = ();
    fn handle(&mut self, msg: MediatorEvent, ctx: &mut Self::Context) -> Self::Result {
        if let Some(mediator) = &self.mediator {
            match msg {
                MediatorEvent::AlreadyConnected => {
                    self.handler.do_send(RakClientEvent::ConnectionFailed(
                        ConnectionFailedReason::AlreadyConnected,
                    ));
                }
                MediatorEvent::DifferentVersion => {
                    self.handler.do_send(RakClientEvent::ConnectionFailed(
                        ConnectionFailedReason::DifferentVersion,
                    ));
                }
                MediatorEvent::Success(mtu) => {
                    let mut session = Session::new(
                        self.remote.unwrap(),
                        mtu,
                        self.udp.clone().recipient::<SendUdp>(),
                        ctx.address(),
                    );
                    let request = ConnectionRequest::new(
                        self.guid,
                        time().try_into().unwrap_or_default(),
                        false,
                    );
                    session.send_system_packet(request, Reliability::Reliable);
                    self.session = Some(session);
                    self.disconnect_handle = Some(
                        ctx.run_later(Duration::from_secs(5), |me, _ctx| me.connection_timeout()),
                    );
                    self.tick_handle =
                        Some(ctx.run_interval(Duration::from_millis(10), |me, ctx| {
                            me.update(ctx);
                        }));
                }
                MediatorEvent::Timeout => {
                    self.handler.do_send(RakClientEvent::ConnectionFailed(
                        ConnectionFailedReason::Timeout,
                    ));
                }
            }
            mediator.do_send(TerminateMediator);
            self.mediator = None;
        }
    }
}

impl<T> Handler<ReceivedDatagram> for RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    type Result = ();
    fn handle(&mut self, msg: ReceivedDatagram, ctx: &mut Self::Context) -> Self::Result {
        match msg.0.data[0] {
            ConnectionRequestAccepted::ID => {
                if let Some(handle) = self.disconnect_handle {
                    let accept =
                        unwrap_or_return!(decode::<ConnectionRequestAccepted>(&msg.0.data));
                    ctx.cancel_future(handle);
                    self.disconnect_handle = None;
                    let connected = NewIncomingConnection {
                        server_address: self.remote.unwrap(),
                        request_timestamp: accept.request_timestamp,
                        accepted_timestamp: accept.request_timestamp,
                    };
                    self.session
                        .as_mut()
                        .unwrap()
                        .send_system_packet(connected, Reliability::ReliableOrdered);
                    self.session.as_mut().unwrap().force_flush();
                    self.handler.do_send(RakClientEvent::Connected);
                }
            }
            _ => {
                self.handler.do_send(RakClientEvent::Packet(msg.0.data));
            }
        }
    }
}

impl<T> Handler<SessionEnd> for RakClient<T>
where
    T: Actor,
    T: Handler<RakClientEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakClientEvent>,
{
    type Result = ();
    fn handle(&mut self, _msg: SessionEnd, _ctx: &mut Self::Context) -> Self::Result {
        self.remote = None;
        self.session = None;
        self.handler.do_send(RakClientEvent::Disconnected);
    }
}

pub(crate) struct ClientMediator {
    udp: Recipient<SendUdp>,
    parent: Recipient<MediatorEvent>,
    guid: u64,
    address: SocketAddr,

    disconnect_timer: Option<SpawnHandle>,

    request1_count: u32,
    next_request1_handle: Option<SpawnHandle>,
}

impl ClientMediator {
    fn new(
        udp: Recipient<SendUdp>,
        parent: Recipient<MediatorEvent>,
        guid: u64,
        address: SocketAddr,
    ) -> Addr<Self> {
        Self::create(|_ctx| Self {
            udp,
            parent,
            guid,
            address,
            disconnect_timer: None,
            request1_count: 0,
            next_request1_handle: None,
        })
    }
    fn request1(&mut self, ctx: &mut Context<Self>) {
        let mtu_size;
        if self.request1_count < 4 {
            mtu_size = 1496;
        } else if (4..8).contains(&self.request1_count) {
            mtu_size = 1204;
        } else if (8..13).contains(&self.request1_count) {
            mtu_size = 584;
        } else {
            self.timeout(ctx);
            return;
        }
        self.request1_count += 1;

        let request1 = OpenConnectionRequest1::new(RAKNET_PROTOCOL_VERSION, mtu_size);

        unwrap_or_return!(self.udp.do_send(SendUdp(UdpPacket {
            bytes: encode(request1),
            addr: self.address,
        })));
        self.next_request1_handle =
            Some(ctx.run_later(Duration::from_millis(510), |me, ctx| me.request1(ctx)));
    }
    fn request2(&mut self, mtu: u16, ctx: &mut Context<Self>) {
        let request2 = OpenConnectionRequest2::new(self.address, mtu, self.guid);
        unwrap_or_return!(self.udp.do_send(SendUdp(UdpPacket {
            bytes: encode(request2),
            addr: self.address,
        })));
        self.next_request1_handle =
            Some(ctx.run_later(Duration::from_millis(510), |me, ctx| me.request1(ctx)));
    }
    fn success(&mut self, mtu: u16, ctx: &mut Context<Self>) {
        if let Some(handle) = self.disconnect_timer {
            ctx.cancel_future(handle);
            self.disconnect_timer = None;
            self.event(MediatorEvent::Success(mtu), ctx);
        }
    }
    fn timeout(&mut self, ctx: &mut Context<Self>) {
        self.event(MediatorEvent::Timeout, ctx);
    }
    fn already_connected(&mut self, ctx: &mut Context<Self>) {
        self.event(MediatorEvent::AlreadyConnected, ctx);
    }
    fn different_version(&mut self, ctx: &mut Context<Self>) {
        self.event(MediatorEvent::DifferentVersion, ctx);
    }

    fn event(&mut self, event: MediatorEvent, ctx: &mut Context<Self>) {
        self.parent.do_send(event).unwrap_or_else(|e| {
            if let SendError::Closed(_event) = e {
                ctx.terminate()
            }
        });
    }
}

impl Actor for ClientMediator {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        self.disconnect_timer = Some(ctx.run_later(Duration::from_secs(10), |me, ctx| {
            me.timeout(ctx);
        }));
        self.request1(ctx);
    }
}

impl Handler<ReceivedUdp> for ClientMediator {
    type Result = ();
    fn handle(&mut self, msg: ReceivedUdp, ctx: &mut Self::Context) -> Self::Result {
        match msg.0.bytes[0] {
            OpenConnectionReply1::ID => {
                if let Some(handle) = self.next_request1_handle {
                    let reply1 = unwrap_or_return!(decode::<OpenConnectionReply1>(&msg.0.bytes));
                    ctx.cancel_future(handle);
                    self.next_request1_handle = None;
                    self.request2(reply1.mtu_size, ctx);
                }
            }
            OpenConnectionReply2::ID => {
                if let Some(handle) = self.next_request1_handle {
                    let reply2 = unwrap_or_return!(decode::<OpenConnectionReply2>(&msg.0.bytes));
                    ctx.cancel_future(handle);
                    self.next_request1_handle = None;
                    self.success(reply2.mtu + 96, ctx);
                }
            }
            AlreadyConnected::ID => {
                if let Some(handle) = self.next_request1_handle {
                    let _packet = unwrap_or_return!(decode::<AlreadyConnected>(&msg.0.bytes));
                    ctx.cancel_future(handle);
                    self.next_request1_handle = None;
                    self.already_connected(ctx);
                }
            }
            IncompatibleProtocolVersion::ID => {
                if let Some(handle) = self.next_request1_handle {
                    let _packet =
                        unwrap_or_return!(decode::<IncompatibleProtocolVersion>(&msg.0.bytes));
                    ctx.cancel_future(handle);
                    self.next_request1_handle = None;
                    self.different_version(ctx);
                }
            }
            _ => {}
        }
    }
}

impl Handler<TerminateMediator> for ClientMediator {
    type Result = ();
    fn handle(&mut self, _msg: TerminateMediator, ctx: &mut Self::Context) -> Self::Result {
        ctx.terminate();
    }
}

#[derive(Message)]
#[rtype(result = "()")]
enum MediatorEvent {
    Timeout,
    DifferentVersion,
    AlreadyConnected,
    Success(u16),
}

#[derive(Message)]
#[rtype(result = "()")]
struct TerminateMediator;
