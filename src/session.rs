use std::{net::SocketAddr, time::Instant};

use actix::{dev::ToEnvelope, prelude::*};
use bytes::BytesMut;

use crate::{
    macros::unwrap_or_return,
    packet::ACKQueue,
    packetqueue::PacketQueue,
    packets::*,
    receivedqueue::ReceivedQueue,
    udp::{ReceivedUdp, SendUdp, UdpPacket},
};

const DATAGRAM_FLAG: u8 = 0x80;

const ACK_FLAG: u8 = 0x40;

const NACK_FLAG: u8 = 0x20;

pub(crate) struct Session<M>
where
    M: Actor,
    M: Handler<ReceivedDatagram>,
    M: Handler<SessionEnd>,
    <M as actix::Actor>::Context: ToEnvelope<M, ReceivedDatagram>,
    <M as actix::Actor>::Context: ToEnvelope<M, SessionEnd>,
{
    ack_queue: ACKQueue,
    packet_queue: PacketQueue,
    received: ReceivedQueue,
    udp: Recipient<SendUdp>,
    parent: Addr<M>,
    addr: SocketAddr,
    mtu: u16,
    message_index: u32,
    order_index: u32,
    split_id: u16,
    last_ping: Instant,
    last_receive: Instant,
    disconnected: bool,
}

impl<M> Session<M>
where
    M: Actor,
    M: Handler<ReceivedDatagram>,
    M: Handler<SessionEnd>,
    <M as actix::Actor>::Context: ToEnvelope<M, ReceivedDatagram>,
    <M as actix::Actor>::Context: ToEnvelope<M, SessionEnd>,
{
    pub fn new(addr: SocketAddr, mtu: u16, udp: Recipient<SendUdp>, parent: Addr<M>) -> Self {
        Self {
            ack_queue: ACKQueue::new(),
            packet_queue: PacketQueue::new(mtu),
            received: ReceivedQueue::new(),
            udp,
            parent,
            addr,
            mtu,
            message_index: 0,
            order_index: 0,
            split_id: 0,
            last_ping: Instant::now(),
            last_receive: Instant::now(),
            disconnected: false,
        }
    }
    pub fn update(&mut self) {
        if !self.disconnected {
            self.flush_queue();
            self.flush_ack();
            if Instant::now().duration_since(self.last_receive).as_secs() > 10 {
                self.disconnect();
            }
            if Instant::now().duration_since(self.last_ping).as_millis() > 3000 {
                self.last_ping = Instant::now();
                self.send_ping();
            }
        }
    }
    pub fn force_flush(&mut self) {
        self.flush_queue();
    }
    fn flush_queue(&mut self) {
        for send_able in self.packet_queue.get_packet() {
            let frame_set = send_able.encode();
            unwrap_or_return!(self.udp.do_send(SendUdp(UdpPacket {
                bytes: frame_set,
                addr: self.addr,
            })));
        }
    }
    fn flush_ack(&mut self) {
        let (acks, nacks) = self.ack_queue.clear();
        for ack in acks {
            self.send_ack(ack);
        }
        for miss in nacks {
            self.send_nack(miss);
        }
    }
    fn send_ack(&mut self, packet: (u32, u32)) {
        let ack = Ack::new(packet);
        let buff = encode(ack);
        unwrap_or_return!(self.udp.do_send(SendUdp(UdpPacket {
            bytes: buff,
            addr: self.addr,
        })));
    }
    fn handle_ack(&mut self, buff: &BytesMut) {
        let ack = unwrap_or_return!(decode::<Ack>(buff));
        for sequence in ack.get_all() {
            self.packet_queue.received(sequence);
        }
    }
    fn handle_nack(&mut self, buff: &BytesMut) {
        let nack = unwrap_or_return!(decode::<Nack>(buff));
        for sequence in nack.get_all() {
            self.packet_queue.resend(sequence)
        }
    }
    fn handle_datagram(&mut self, buff: &BytesMut) {
        let frame_set = unwrap_or_return!(FrameSet::decode(buff));
        self.ack_queue.add(frame_set.sequence_number);
        for frame in frame_set.datas {
            self.receive_packet(frame)
        }
    }
    fn send_nack(&mut self, miss: u32) {
        let nack = Nack::new((miss, miss));
        unwrap_or_return!(self.udp.do_send(SendUdp(UdpPacket {
            bytes: encode(nack),
            addr: self.addr,
        })));
    }
    fn send_ping(&mut self) {
        let connected_ping = ConnectedPing::new(time() as i64);
        let buff = encode(connected_ping);
        let frame = Frame::new(Reliability::Unreliable, buff);
        self.send(frame);
    }
    fn receive_packet(&mut self, frame: Frame) {
        if !frame.reliability.sequenced_or_ordered() {
            self.handle_packet(frame);
        } else {
            self.received.add(frame);
            for packet in self.received.get_all() {
                self.handle_packet(packet);
            }
        }
    }
    fn handle_packet(&mut self, frame: Frame) {
        self.last_receive = Instant::now();
        if frame.data[0] == ConnectedPing::ID {
            let ping = unwrap_or_return!(decode::<ConnectedPing>(&frame.data));
            self.handle_connected_ping(ping);
            return;
        } else if frame.data[0] == ConnectedPong::ID {
            let _pong = unwrap_or_return!(decode::<ConnectedPong>(&frame.data));
            return;
        } else if frame.data[0] == Disconnected::ID {
            let _disconnect = unwrap_or_return!(decode::<Disconnected>(&frame.data));
            self.end();
            return;
        }
        self.parent.do_send(ReceivedDatagram(frame));
    }

    pub fn handle(&mut self, msg: ReceivedUdp) {
        let header = msg.0.bytes[0];

        if header & ACK_FLAG != 0 {
            self.handle_ack(&msg.0.bytes);
        } else if header & NACK_FLAG != 0 {
            self.handle_nack(&msg.0.bytes);
        } else if header & DATAGRAM_FLAG != 0 {
            self.handle_datagram(&msg.0.bytes);
        }
    }

    fn handle_connected_ping(&mut self, ping: ConnectedPing) {
        let pong = ConnectedPong::new(ping.client_timestamp, time() as i64);
        let buff = encode(pong);

        let frame = Frame::new(Reliability::Unreliable, buff);
        self.send(frame);
    }
    fn send(&mut self, packet: Frame) {
        self.packet_queue.add_frame(packet);
    }
    pub fn disconnect(&mut self) {
        let buff = encode(Disconnected {});
        let mut frame = Frame::new(Reliability::ReliableOrdered, buff);
        frame.message_index = self.message_index;
        frame.order_index = self.order_index;
        self.message_index += 1;
        self.order_index += 1;
        self.send(frame);
        self.flush_queue();
        self.end();
    }
    fn end(&mut self) {
        self.parent.do_send(SessionEnd);
        self.disconnected = true;
    }
    pub fn send_system_packet<P: Packet>(&mut self, packet: P, reliability: Reliability) {
        let buff = encode(packet);
        let mut packet = Frame::new(reliability.clone(), buff);
        if reliability.reliable() {
            packet.message_index = self.message_index;
            self.message_index += 1;
        }
        if reliability.sequenced_or_ordered() {
            packet.order_index = self.order_index;
            self.order_index += 1;
        }
        self.send(packet);
    }
    pub fn send_to(&mut self, mut buff: BytesMut) {
        if buff.len() < (self.mtu - 14 - 32).into() {
            let mut frame = Frame::new(Reliability::ReliableOrdered, buff);
            frame.message_index = self.message_index;
            frame.order_index = self.order_index;
            self.message_index += 1;
            self.order_index += 1;
            self.send(frame);
        } else {
            let len = buff.len() as u16;
            let max = self.mtu - 24 - 32 - 5;
            let mut split_len = buff.len() as u16 / max;
            if len % max != 0 {
                split_len += 1;
            }
            for i in 0..split_len {
                let pos;
                if i == split_len - 1 {
                    pos = len % max;
                } else {
                    pos = max;
                }
                let mut frame = Frame::new(Reliability::ReliableOrdered, buff.split_to(pos.into()));
                frame.split = true;
                frame.message_index = self.message_index;
                frame.order_index = self.order_index;
                frame.split_count = split_len as u32;
                frame.split_id = self.split_id;
                frame.split_index = i as u32;
                self.send(frame);
                self.message_index += 1;
            }
            self.split_id += 1;
            self.order_index += 1;
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
pub(crate) struct ReceivedDatagram(pub Frame);

#[derive(Message)]
#[rtype(result = "()")]
pub(crate) struct SessionEnd;

pub(crate) fn time() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap_or(0)
}
