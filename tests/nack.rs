use std::net::SocketAddr;

use actix::prelude::*;
use actix_raknet::{
    client::{ClientHandle, RakClient, RakClientEvent},
    packets::*,
};
use futures::executor::block_on;
struct Client {
    rak_client: ClientHandle,
}

impl Actor for Client {
    type Context = Context<Self>;
}

impl Handler<RakClientEvent> for Client {
    type Result = ();
    fn handle(&mut self, msg: RakClientEvent, _ctx: &mut Self::Context) -> Self::Result {
        if let RakClientEvent::ConnectionFailed(reason) = msg {
            match reason {
                actix_raknet::client::ConnectionFailedReason::AlreadyConnected => {
                    println!("already connected");
                }
                actix_raknet::client::ConnectionFailedReason::DifferentVersion => {
                    println!("different version");
                }
                actix_raknet::client::ConnectionFailedReason::Timeout => {
                    println!("timeout");
                }
            }
            System::current().stop()
        }
    }
}

impl Handler<Connect> for Client {
    type Result = ();

    fn handle(&mut self, msg: Connect, _ctx: &mut Self::Context) -> Self::Result {
        self.rak_client.connect(msg.0);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct Connect(SocketAddr);

async fn creat_client(guid: u64, addr: SocketAddr) -> Addr<Client> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    Client::create(|ctx| {
        let rak_client = RakClient::init(socket, guid, ctx.address());
        Client { rak_client }
    })
}

const DATAGRAM_FLAG: u8 = 0x80;

const NACK_FLAG: u8 = 0x20;

const NEEDS_B_AND_AS_FLAG: u8 = 0x4;

#[test]
fn recv_nack() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        let mut socket = block_on(tokio::net::UdpSocket::bind(server_addr)).unwrap();
        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 1500];
                let (length, source) = socket.recv_from(&mut buff).await.unwrap();
                if buff[0] == OpenConnectionRequest1::ID {
                    let reply = OpenConnectionReply1::new(0, false, 1498);
                    let payload = encode(reply);
                    socket.send_to(&payload, source).await.unwrap();
                }
                if buff[0] == OpenConnectionRequest2::ID {
                    let reply = OpenConnectionReply2::new(0, server_addr, 1498, false);
                    let payload = encode(reply);
                    socket.send_to(&payload, source).await.unwrap();
                }
                if buff[0] & DATAGRAM_FLAG != 0 {
                    let frame_set = FrameSet::decode(&buff[..length]).unwrap();
                    let nack = Nack::new((frame_set.sequence_number, frame_set.sequence_number));
                    let payload = encode(nack);
                    socket.send_to(&payload, source).await.unwrap();
                }
            }
        });

        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(creat_client(114514, client1_addr));
        client1.do_send(Connect(server_addr));
    })
    .unwrap();
}

#[test]
fn send_nack() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:19133".parse().unwrap();
        let mut socket = block_on(tokio::net::UdpSocket::bind(server_addr)).unwrap();
        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 1500];
                let (length, source) = socket.recv_from(&mut buff).await.unwrap();
                if buff[0] == OpenConnectionRequest1::ID {
                    let reply = OpenConnectionReply1::new(0, false, 1498);
                    let payload = encode(reply);
                    socket.send_to(&payload, source).await.unwrap();
                }
                if buff[0] == OpenConnectionRequest2::ID {
                    let reply = OpenConnectionReply2::new(0, server_addr, 1498, false);
                    let payload = encode(reply);
                    socket.send_to(&payload, source).await.unwrap();
                }
                if buff[0] & DATAGRAM_FLAG != 0 {
                    let frame_set = FrameSet::decode(&buff[..length]).unwrap();
                    for frame in frame_set.datas {
                        if frame.data.len() == 0 {
                            continue;
                        }
                        if frame.data[0] == ConnectionRequest::ID {
                            let request = decode::<ConnectionRequest>(&frame.data).unwrap();
                            let accept = encode(ConnectionRequestAccepted::new(
                                source,
                                request.time,
                                time() as i64,
                            ));
                            let frame = Frame::new(Reliability::ReliableOrdered, accept);
                            let frameset = FrameSet {
                                header: DATAGRAM_FLAG | NEEDS_B_AND_AS_FLAG,
                                sequence_number: 1,
                                datas: vec![frame],
                            };
                            let payload = frameset.encode();
                            socket.send_to(&payload, source).await.unwrap();
                        }
                    }
                }
                if buff[0] & NACK_FLAG != 0 {
                    System::current().stop();
                }
            }
        });

        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(creat_client(114514, client1_addr));
        client1.do_send(Connect(server_addr));
    })
    .unwrap();
}

fn time() -> u128 {
    std::convert::TryInto::try_into(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap_or(0)
}
