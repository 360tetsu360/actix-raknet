use std::net::SocketAddr;

use crate::{
    macros::unwrap_or_return,
    packets::{decode, encode, Packet, UnconnectedPing, UnconnectedPong},
    session::time,
    udp::{ReceivedUdp, SendUdp, UdpActor, UdpPacket},
};
use actix::{dev::ToEnvelope, prelude::*};

#[derive(Message)]
#[rtype(result = "()")]
pub struct PingTo(pub SocketAddr);

#[derive(Message)]
#[rtype(result = "()")]
pub struct Pong(pub SocketAddr, pub String);

pub struct RakPing<T>
where
    T: Actor + Handler<Pong>,
    <T as actix::Actor>::Context: ToEnvelope<T, Pong>,
{
    udp: Addr<UdpActor<Self>>,
    handler: Addr<T>,
}

impl<T> RakPing<T>
where
    T: Actor + Handler<Pong>,
    <T as actix::Actor>::Context: ToEnvelope<T, Pong>,
{
    pub fn new(socket: tokio::net::UdpSocket, handler: Addr<T>) -> Addr<Self> {
        Self::create(|ctx| Self {
            udp: UdpActor::new(socket, ctx.address()),
            handler,
        })
    }
}

impl<T> Actor for RakPing<T>
where
    T: Actor + Handler<Pong>,
    <T as actix::Actor>::Context: ToEnvelope<T, Pong>,
{
    type Context = Context<Self>;
}

impl<T> Handler<ReceivedUdp> for RakPing<T>
where
    T: Actor + Handler<Pong>,
    <T as actix::Actor>::Context: ToEnvelope<T, Pong>,
{
    type Result = ();
    fn handle(&mut self, msg: ReceivedUdp, _ctx: &mut Self::Context) -> Self::Result {
        if msg.0.bytes[0] == UnconnectedPong::ID {
            let pong = unwrap_or_return!(decode::<UnconnectedPong>(&msg.0.bytes));
            self.handler.do_send(Pong(msg.0.addr, pong.motd));
        }
    }
}

impl<T> Handler<PingTo> for RakPing<T>
where
    T: Actor + Handler<Pong>,
    <T as actix::Actor>::Context: ToEnvelope<T, Pong>,
{
    type Result = ();
    fn handle(&mut self, msg: PingTo, _ctx: &mut Self::Context) -> Self::Result {
        let ping = UnconnectedPing::new(time() as i64, 0x0);
        let payload = encode(ping);
        self.udp.do_send(SendUdp(UdpPacket {
            bytes: payload,
            addr: msg.0,
        }));
    }
}
