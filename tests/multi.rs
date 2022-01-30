use std::net::SocketAddr;

use actix::prelude::*;
use bytes::BytesMut;
use futures::executor::block_on;
use actix_raknet::{
    client::{RakClient, RakClientEvent, RakClientMsg},
    server::{RakServer, RakServerEvent},
};
struct Client {
    server_addr : SocketAddr,
    connected_count : u32,
    rak_client: Addr<RakClient<Self>>,
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
                let packet: &[u8] = b"Hello Server!";
                self.rak_client
                    .do_send(RakClientMsg::Packet(BytesMut::from(packet)));
                self.connected_count += 1;
            }
            RakClientEvent::Packet(_) => {}
            RakClientEvent::Disconnected => {
                dbg!(self.connected_count);
                if self.connected_count == 10 {
                    System::current().stop();
                    return;
                }
                self.rak_client.do_send(RakClientMsg::Connect(self.server_addr));
            }
        }
    }
}

impl Handler<Connect> for Client {
    type Result = ();

    fn handle(&mut self, msg: Connect, _ctx: &mut Self::Context) -> Self::Result {
        self.rak_client.do_send(RakClientMsg::Connect(msg.0));
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
    fn handle(&mut self, msg: RakServerEvent, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakServerEvent::Connected(_) => {}
            RakServerEvent::Packet(p, _) => {
                p.disconnect()
            }
            RakServerEvent::Disconnected(_, _) => {}
        }
    }
}

async fn create_client(guid: u64, addr: SocketAddr) -> Addr<Client> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    Client::create(|ctx| {
        let rak_client = RakClient::new(socket, guid, ctx.address());
        Client { rak_client,connected_count : 0, server_addr }
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
fn multi() {
    System::run(||{
        let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        block_on(create_server(0x1919, server_addr, "MCPE;§5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned()));
    
        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(create_client(114514, client1_addr));
        client1.do_send(Connect(server_addr));
        
    }).unwrap();
}
