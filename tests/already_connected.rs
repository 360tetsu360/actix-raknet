use std::net::SocketAddr;

use actix::prelude::*;
use actix_raknet::{
    client::{ClientHandle, RakClient, RakClientEvent},
    packets::{encode, IncompatibleProtocolVersion, OpenConnectionRequest1, Packet},
    server::{RakServer, RakServerEvent},
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

struct Server;
impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<RakServerEvent> for Server {
    type Result = ();
    fn handle(&mut self, _msg: RakServerEvent, _: &mut Self::Context) -> Self::Result {}
}

async fn create_client(guid: u64, addr: SocketAddr) -> Addr<Client> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    Client::create(|ctx| {
        let rak_client = RakClient::init(socket, guid, ctx.address(), System::current().arbiter());
        Client { rak_client }
    })
}

async fn create_server(guid: u64, addr: SocketAddr, motd: String) -> Addr<Server> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    Server::create(|ctx| {
        RakServer::new(socket, guid, motd, ctx.address(), 1);
        Server
    })
}

#[test]
fn already_connected() {
    System::run(||{
        let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        block_on(create_server(0x1919, server_addr, "MCPE;??5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned()));

        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(create_client(114514, client1_addr));
        client1.do_send(Connect(server_addr));

        let client2_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client2 = block_on(create_client(114514, client2_addr));
        client2.do_send(Connect(server_addr));
    }).unwrap();
}

#[test]
fn different_version_server() {
    System::run(||{
        let server_addr: SocketAddr = "127.0.0.1:19133".parse().unwrap();
        block_on(create_server(0x1919, server_addr, "MCPE;??5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned()));

        tokio::spawn(async move {
            let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
            let mut socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
            let connectionreq = OpenConnectionRequest1::new(0xFF, 1498);
            let payload = encode(connectionreq);
            socket.send_to(&payload, server_addr).await.unwrap();

            let mut buff = [0u8;1500];
            let _ = socket.recv(&mut buff).await.unwrap();
            if buff[0] == IncompatibleProtocolVersion::ID {
                System::current().stop();
            }
        });
    }).unwrap();
}
