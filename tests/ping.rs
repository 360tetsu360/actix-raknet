use actix::prelude::*;
use actix_raknet::{
    ping::{PingTo, Pong, RakPing},
    server::{RakServer, RakServerEvent},
};
use futures::executor::block_on;
use std::net::SocketAddr;
struct Ping {
    ping: Addr<RakPing<Self>>,
}

impl Actor for Ping {
    type Context = Context<Self>;
}

impl Handler<Pong> for Ping {
    type Result = ();
    fn handle(&mut self, msg: Pong, _ctx: &mut Self::Context) -> Self::Result {
        println!("{} {}", msg.0, msg.1);
        System::current().stop();
    }
}

impl Handler<PingTo> for Ping {
    type Result = ();
    fn handle(&mut self, msg: PingTo, _ctx: &mut Self::Context) -> Self::Result {
        self.ping.do_send(msg);
    }
}

struct Server;
impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<RakServerEvent> for Server {
    type Result = ();
    fn handle(&mut self, _msg: RakServerEvent, _: &mut Self::Context) -> Self::Result {}
}

async fn create_server(guid: u64, addr: SocketAddr, motd: String) -> Addr<Server> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    Server::create(|ctx| {
        RakServer::new(socket, guid, motd, ctx.address(), 1);
        Server
    })
}

async fn create_ping(addr: SocketAddr) -> Addr<Ping> {
    let socket = tokio::net::UdpSocket::bind(addr).await.unwrap();
    Ping::create(|ctx| {
        let ping = RakPing::new(socket, ctx.address());
        Ping { ping }
    })
}

#[test]
fn ping() {
    System::run(||{
        let server_addr: SocketAddr = "127.0.0.1:19144".parse().unwrap();
        block_on(create_server(0x1919, server_addr, "MCPE;§5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned()));

        let ping_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let ping = block_on(create_ping(ping_addr));
        ping.do_send(PingTo(server_addr));
    }).unwrap();
}
