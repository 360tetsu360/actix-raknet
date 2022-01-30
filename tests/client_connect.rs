use std::net::SocketAddr;

use actix::prelude::*;
use actix_raknet::{
    client::{ClientHandle, RakClient, RakClientEvent},
    packets::{
        encode, AlreadyConnected, IncompatibleProtocolVersion, OpenConnectionReply1,
        OpenConnectionReply2, OpenConnectionRequest1, OpenConnectionRequest2, Packet,
    },
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
        }
        System::current().stop()
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

#[test]
fn mtu() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut socket = block_on(tokio::net::UdpSocket::bind(server_addr)).unwrap();
        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 1500];
                socket.recv_from(&mut buff).await.unwrap();
            }
        });

        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(creat_client(114514, client1_addr));
        client1.do_send(Connect(server_addr));
    })
    .unwrap();
}

#[test]
fn connection_failed() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut socket = block_on(tokio::net::UdpSocket::bind(server_addr)).unwrap();
        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 1500];
                let (_length, source) = socket.recv_from(&mut buff).await.unwrap();
                if buff[0] == OpenConnectionRequest1::ID {
                    let reply = OpenConnectionReply1::new(0, false, 1498);
                    let payload = encode(reply);
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
fn different_version() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut socket = block_on(tokio::net::UdpSocket::bind(server_addr)).unwrap();
        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 1500];
                let (_length, source) = socket.recv_from(&mut buff).await.unwrap();
                if buff[0] == OpenConnectionRequest1::ID {
                    let reply = IncompatibleProtocolVersion::new(0, 0);
                    let payload = encode(reply);
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
fn already_connected() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut socket = block_on(tokio::net::UdpSocket::bind(server_addr)).unwrap();
        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 1500];
                let (_length, source) = socket.recv_from(&mut buff).await.unwrap();
                if buff[0] == OpenConnectionRequest1::ID {
                    let reply = AlreadyConnected::new(0);
                    let payload = encode(reply);
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
fn timeout() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let mut socket = block_on(tokio::net::UdpSocket::bind(server_addr)).unwrap();
        tokio::spawn(async move {
            loop {
                let mut buff = [0u8; 1500];
                let (_length, source) = socket.recv_from(&mut buff).await.unwrap();
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
            }
        });

        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(creat_client(114514, client1_addr));
        client1.do_send(Connect(server_addr));
    })
    .unwrap();
}
