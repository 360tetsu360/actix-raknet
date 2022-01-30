use actix::{dev::ToEnvelope, io::SinkWrite, prelude::*};
use bytes::BytesMut;
use futures::StreamExt;
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use tokio_util::{codec::BytesCodec, udp::UdpFramed};

use crate::{
    macros::unwrap_or_return,
    packets::*,
    session::{time, ReceivedDatagram, Session, SessionEnd},
    udp::{ReceivedUdp, SendUdp, UdpActor, UdpPacket},
    RAKNET_PROTOCOL_VERSION,
};

#[derive(Clone)]
pub struct ConnectionHandle {
    addr: Addr<ServerConn>,
    pub address: SocketAddr,
    pub guid: u64,
}
impl ConnectionHandle {
    pub fn send(&self, bytes: BytesMut) {
        self.addr.do_send(SendPacket(bytes));
    }
    pub fn disconnect(&self) {
        self.addr.do_send(DisconnectConn);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub enum RakServerEvent {
    Connected(ConnectionHandle),
    Packet(ConnectionHandle, BytesMut),
    Disconnected(SocketAddr, u64),
}

pub struct RakServer<T>
where
    T: Actor,
    T: Handler<RakServerEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakServerEvent>,
{
    udp: Addr<UdpActor<Self>>,
    handler: Addr<T>,
    conns: HashMap<SocketAddr, Addr<ServerConn>>,
    connected_id: Vec<u64>,
    motd: String,
    guid: u64,
}

impl<T> RakServer<T>
where
    T: Actor,
    T: Handler<RakServerEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakServerEvent>,
{
    pub fn new(
        socket: tokio::net::UdpSocket,
        guid: u64,
        motd: String,
        handler: Addr<T>,
    ) -> Addr<Self> {
        let (sink, stream) = UdpFramed::new(socket, BytesCodec::new()).split();
        Self::create(|ctx| Self {
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
            handler,
            conns: HashMap::new(),
            connected_id: vec![],
            motd,
            guid,
        })
    }
}

impl<T> Actor for RakServer<T>
where
    T: Actor,
    T: Handler<RakServerEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakServerEvent>,
{
    type Context = Context<Self>;
}

impl<T> Handler<ReceivedUdp> for RakServer<T>
where
    T: Actor,
    T: Handler<RakServerEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakServerEvent>,
{
    type Result = ();
    fn handle(&mut self, msg: ReceivedUdp, ctx: &mut Self::Context) -> Self::Result {
        if let Some(conn) = self.conns.get(&msg.0.addr) {
            conn.do_send(msg);
            return;
        }

        let buff: &[u8] = &msg.0.bytes;
        match buff[0] {
            UnconnectedPing::ID => {
                let ping = unwrap_or_return!(decode::<UnconnectedPing>(buff));
                let pong = UnconnectedPong::new(ping.time, self.guid, self.motd.clone());
                self.udp.do_send(SendUdp(UdpPacket {
                    bytes: encode(pong),
                    addr: msg.0.addr,
                }));
            }
            OpenConnectionRequest1::ID => {
                let request1 = unwrap_or_return!(decode::<OpenConnectionRequest1>(buff));
                if request1.protocol_version != RAKNET_PROTOCOL_VERSION {
                    let protocol_version =
                        IncompatibleProtocolVersion::new(RAKNET_PROTOCOL_VERSION, self.guid);
                    self.udp.do_send(SendUdp(UdpPacket {
                        bytes: encode(protocol_version),
                        addr: msg.0.addr,
                    }));
                }
                let reply = OpenConnectionReply1::new(self.guid, false, request1.mtu_size);
                self.udp.do_send(SendUdp(UdpPacket {
                    bytes: encode(reply),
                    addr: msg.0.addr,
                }));
            }
            OpenConnectionRequest2::ID => {
                let request2 = unwrap_or_return!(decode::<OpenConnectionRequest2>(buff));

                if self.connected_id.contains(&request2.guid) {
                    let already_connected = AlreadyConnected::new(request2.guid);
                    let data = encode(already_connected);
                    self.udp.do_send(SendUdp(UdpPacket {
                        bytes: data,
                        addr: msg.0.addr,
                    }));
                    return;
                }

                let reply2 = OpenConnectionReply2::new(self.guid, msg.0.addr, request2.mtu, false);
                self.udp.do_send(SendUdp(UdpPacket {
                    bytes: encode(reply2),
                    addr: msg.0.addr,
                }));
                self.conns.insert(
                    msg.0.addr,
                    ServerConn::new(
                        self.udp.clone().recipient::<SendUdp>(),
                        request2.mtu,
                        request2.guid,
                        msg.0.addr,
                        self.handler.clone().recipient::<RakServerEvent>(),
                        ctx.address().recipient::<ConnectionEnd>(),
                    ),
                );
                self.connected_id.push(request2.guid);
            }
            _ => {}
        }
    }
}

impl<T> Handler<ConnectionEnd> for RakServer<T>
where
    T: Actor,
    T: Handler<RakServerEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakServerEvent>,
{
    type Result = ();
    fn handle(&mut self, msg: ConnectionEnd, _ctx: &mut Self::Context) -> Self::Result {
        self.conns.remove(&msg.0);
        if self.connected_id.contains(&msg.1) {
            let index = self.connected_id.iter().position(|x| *x == msg.1).unwrap();
            self.connected_id.remove(index);
        }
    }
}

impl<T> Handler<SetMotd> for RakServer<T>
where
    T: Actor,
    T: Handler<RakServerEvent>,
    <T as actix::Actor>::Context: ToEnvelope<T, RakServerEvent>,
{
    type Result = ();
    fn handle(&mut self, msg: SetMotd, _ctx: &mut Self::Context) -> Self::Result {
        self.motd = msg.0;
    }
}

pub(crate) struct ServerConn {
    session: Session<Self>,
    handler: Recipient<RakServerEvent>,
    server: Recipient<ConnectionEnd>,
    guid: u64,
    addr: SocketAddr,
    disconnect_handle: Option<SpawnHandle>,
}

impl ServerConn {
    pub fn new(
        udp: Recipient<SendUdp>,
        mtu: u16,
        guid: u64,
        addr: SocketAddr,
        handler: Recipient<RakServerEvent>,
        server: Recipient<ConnectionEnd>,
    ) -> Addr<Self> {
        ServerConn::create(|ctx| Self {
            session: Session::<Self>::new(addr, mtu, udp, ctx.address()),
            handler,
            server,
            guid,
            addr,
            disconnect_handle: Some(ctx.run_later(Duration::from_secs(5), |me, _ctx| {
                me.disconnect();
            })),
        })
    }
    fn disconnect(&mut self) {
        self.session.disconnect()
    }
}

impl Actor for ServerConn {
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_millis(10), |me, _ctx| {
            me.session.update();
        });
    }
}

impl Handler<ReceivedUdp> for ServerConn {
    type Result = ();
    fn handle(&mut self, msg: ReceivedUdp, _ctx: &mut Self::Context) -> Self::Result {
        self.session.handle(msg);
    }
}

impl Handler<ReceivedDatagram> for ServerConn {
    type Result = ();
    fn handle(&mut self, msg: ReceivedDatagram, ctx: &mut Self::Context) -> Self::Result {
        if let Some(handle) = self.disconnect_handle {
            match msg.0.data[0] {
                ConnectionRequest::ID => {
                    let request = unwrap_or_return!(decode::<ConnectionRequest>(&msg.0.data));
                    let accept = ConnectionRequestAccepted::new(
                        self.addr,
                        request.time,
                        time().try_into().unwrap_or_default(),
                    );
                    self.session
                        .send_system_packet(accept, Reliability::ReliableOrdered);
                }
                NewIncomingConnection::ID => {
                    let _connected =
                        unwrap_or_return!(decode::<NewIncomingConnection>(&msg.0.data));
                    let my_handle = ConnectionHandle {
                        addr: ctx.address(),
                        address: self.addr,
                        guid: self.guid,
                    };
                    self.handler
                        .do_send(RakServerEvent::Connected(my_handle))
                        .unwrap();
                    ctx.cancel_future(handle);
                    self.disconnect_handle = None;
                }
                _ => {}
            }
            return;
        }
        let my_handle = ConnectionHandle {
            addr: ctx.address(),
            address: self.addr,
            guid: self.guid,
        };
        self.handler
            .do_send(RakServerEvent::Packet(my_handle, msg.0.data))
            .unwrap();
    }
}

impl Handler<SessionEnd> for ServerConn {
    type Result = ();
    fn handle(&mut self, _msg: SessionEnd, ctx: &mut Self::Context) -> Self::Result {
        self.server
            .do_send(ConnectionEnd(self.addr, self.guid))
            .unwrap();
        self.handler
            .do_send(RakServerEvent::Disconnected(self.addr, self.guid))
            .unwrap();
        ctx.terminate();
    }
}

impl Handler<SendPacket> for ServerConn {
    type Result = ();
    fn handle(&mut self, msg: SendPacket, _ctx: &mut Self::Context) -> Self::Result {
        self.session.send_to(msg.0);
    }
}

impl Handler<DisconnectConn> for ServerConn {
    type Result = ();
    fn handle(&mut self, _msg: DisconnectConn, _ctx: &mut Self::Context) -> Self::Result {
        self.disconnect()
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub(crate) struct SendPacket(BytesMut);

#[derive(Message)]
#[rtype(result = "()")]
pub(crate) struct DisconnectConn;

#[derive(Message)]
#[rtype(result = "()")]
pub(crate) struct ConnectionEnd(SocketAddr, u64);

#[derive(Message)]
#[rtype(result = "()")]
pub struct SetMotd(pub String);
