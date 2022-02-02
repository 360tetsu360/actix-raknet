use actix::prelude::*;
use actix_raknet::ping::{PingTo, Pong, RakPing};
use std::net::{SocketAddr, ToSocketAddrs};
struct Raknet {
    ping: Addr<RakPing<Self>>,
}

impl Actor for Raknet {
    type Context = Context<Self>;
}

impl Handler<Pong> for Raknet {
    type Result = ();
    fn handle(&mut self, msg: Pong, _ctx: &mut Self::Context) -> Self::Result {
        println!("{} {}", msg.0, msg.1);
    }
}

impl Handler<PingTo> for Raknet {
    type Result = ();
    fn handle(&mut self, msg: PingTo, _ctx: &mut Self::Context) -> Self::Result {
        self.ping.do_send(msg);
    }
}

#[actix_rt::main]
async fn main() {
    let local_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let socket = tokio::net::UdpSocket::bind(local_addr).await.unwrap();
    let handler = Raknet::create(|ctx| {
        let ping = RakPing::new(socket, ctx.address());
        Raknet { ping }
    });

    let remote = "127.0.0.1:19132"
        .to_socket_addrs()
        .unwrap()
        .collect::<Vec<SocketAddr>>();

    handler.do_send(PingTo(remote.first().unwrap().clone()));
    actix_rt::Arbiter::local_join().await;
}
