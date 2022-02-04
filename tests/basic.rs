use std::net::SocketAddr;

use actix::prelude::*;
use actix_raknet::{
    client::{ClientHandle, RakClient, RakClientEvent},
    server::{RakServer, RakServerEvent},
};
use bytes::BytesMut;
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
        match msg {
            RakClientEvent::ConnectionFailed(_) => {}
            RakClientEvent::Connected => {
                let packet: &[u8] = &[0xfeu8; 4800];
                self.rak_client.packet(BytesMut::from(packet));
            }
            RakClientEvent::Packet(_) => self.rak_client.disconnect(),
            RakClientEvent::Disconnected => {}
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

struct Server;
impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<RakServerEvent> for Server {
    type Result = ();
    fn handle(&mut self, msg: RakServerEvent, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakServerEvent::Connected(_) => {}
            RakServerEvent::Packet(p, _) => {
                ctx.run_later(std::time::Duration::from_secs(5), move |_me, _ctx| {
                    let packet: &[u8] = b"Hello";
                    p.send(BytesMut::from(packet));
                });
            }
            RakServerEvent::Disconnected(_, _) => {
                System::current().stop();
            }
        }
    }
}

async fn create_client(guid: u64, addr: SocketAddr) -> Addr<Client> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    Client::create(|ctx| {
        let rak_client = RakClient::init(socket, guid, ctx.address());
        Client { rak_client }
    })
}

async fn create_server(guid: u64, addr: SocketAddr, motd: String) -> Addr<Server> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    Server::create(|ctx| {
        RakServer::new(socket, guid, motd, ctx.address());
        Server
    })
}

#[test]
fn basic() {
    System::run(||{
        let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        block_on(create_server(0x1919, server_addr, "MCPE;§5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned()));
        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(create_client(114514, client1_addr));
        client1.do_send(Connect(server_addr));

    }).unwrap();
}
