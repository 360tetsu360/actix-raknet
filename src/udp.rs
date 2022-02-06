use std::{net::SocketAddr, sync::Arc};

use actix::{dev::ToEnvelope, prelude::*};
use bytes::BytesMut;
use tokio::{
    net::udp::{RecvHalf, SendHalf},
    sync::Mutex,
};

use crate::macros::unwrap_or_return;

#[derive(Message)]
#[rtype(result = "()")]
pub struct UdpPacket {
    pub bytes: BytesMut,
    pub addr: SocketAddr,
}

#[derive(Message)]
#[rtype(result = "bool")] //continue ?
struct SocketErr(std::io::Error);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReceivedUdp(pub UdpPacket);

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendUdp(pub UdpPacket);

pub struct UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    sender: Arc<Mutex<SendHalf>>,
    receiver: Option<RecvHalf>,
    recv_handle: Option<SpawnHandle>,
    handler: Addr<T>,
}

impl<T> UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    pub fn new(socket: tokio::net::UdpSocket, handler: Addr<T>, arbiter: &Arbiter) -> Addr<Self> {
        let (r, s) = socket.split();
        Self::start_in_arbiter(arbiter, |_ctx| Self {
            sender: Arc::new(Mutex::new(s)),
            receiver: Some(r),
            recv_handle: None,
            handler,
        })
    }
}

impl<T> Actor for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        let mut r = self.receiver.take().unwrap();
        let address = ctx.address();
        let udp_future = async move {
            loop {
                let mut buff = BytesMut::with_capacity(2048);
                buff.resize(2048, 0);
                let (len, source) = match r.recv_from(&mut buff).await {
                    Ok(p) => p,
                    Err(e) => {
                        if address.send(SocketErr(e)).await.unwrap() {
                            continue;
                        } else {
                            break;
                        }
                    }
                };
                if len != 0 {
                    buff.truncate(len);
                    address.do_send(UdpPacket {
                        bytes: buff,
                        addr: source,
                    });
                }
            }
        }
        .into_actor(self);
        self.recv_handle = Some(ctx.spawn(udp_future));
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        ctx.cancel_future(self.recv_handle.unwrap());
    }
}

impl<T> Handler<UdpPacket> for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    type Result = ();
    fn handle(&mut self, msg: UdpPacket, _ctx: &mut Self::Context) -> Self::Result {
        self.handler.do_send(ReceivedUdp(msg))
    }
}

impl<T> Handler<SocketErr> for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    type Result = bool;
    fn handle(&mut self, _msg: SocketErr, _ctx: &mut Self::Context) -> Self::Result {
        true
    }
}

impl<T> Handler<SendUdp> for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    type Result = ();
    fn handle(&mut self, msg: SendUdp, _ctx: &mut Self::Context) -> Self::Result {
        let sender = self.sender.clone();
        tokio::spawn(async move {
            unwrap_or_return!(sender.lock().await.send_to(&msg.0.bytes, &msg.0.addr).await);
        });
    }
}
