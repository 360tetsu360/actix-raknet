use std::net::SocketAddr;

use actix::{dev::ToEnvelope, io::SinkWrite, prelude::*};
use bytes::{Bytes, BytesMut};
use futures::stream::SplitSink;
use tokio_util::{codec::BytesCodec, udp::UdpFramed};

type SinkItem = (Bytes, SocketAddr);
type UdpSink = SplitSink<UdpFramed<BytesCodec>, SinkItem>;

pub struct UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    pub sink: SinkWrite<SinkItem, UdpSink>,
    pub addr: Addr<T>,
}

impl<T> Actor for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    type Context = Context<Self>;
    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        Running::Continue
    }
}

impl<T> StreamHandler<UdpPacket> for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    fn handle(&mut self, msg: UdpPacket, _: &mut Context<Self>) {
        self.addr.do_send(ReceivedUdp(msg));
    }
}

impl<T> Handler<SendUdp> for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
    type Result = ();
    fn handle(&mut self, packet: SendUdp, _: &mut <Self as actix::Actor>::Context) -> Self::Result {
        self.sink
            .write((Bytes::from(packet.0.bytes), packet.0.addr));
    }
}

impl<T> actix::io::WriteHandler<std::io::Error> for UdpActor<T>
where
    T: Actor,
    T: Handler<ReceivedUdp>,
    <T as actix::Actor>::Context: ToEnvelope<T, ReceivedUdp>,
{
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct UdpPacket {
    pub bytes: BytesMut,
    pub addr: SocketAddr,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReceivedUdp(pub UdpPacket);

#[derive(Message)]
#[rtype(result = "()")]
pub struct SendUdp(pub UdpPacket);
