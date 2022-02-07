use actix::prelude::*;
use actix_raknet::server::{RakServer, RakServerEvent, SetMotd};
use std::net::SocketAddr;
struct Raknet {
    rak_server: Addr<RakServer<Self>>,
}

impl Actor for Raknet {
    type Context = Context<Self>;
}

impl Handler<RakServerEvent> for Raknet {
    type Result = ();
    fn handle(&mut self, msg: RakServerEvent, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakServerEvent::Connected(handle) => {
                println!("connected {} {}", handle.address, handle.address);
            }
            RakServerEvent::Packet(handle, _bytes) => {
                println!("packet {} {}", handle.address, handle.address);
            }
            RakServerEvent::Disconnected(addr, guid) => {
                let new_motd = "MCPE;Disconnected!;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned();
                self.rak_server.do_send(SetMotd(new_motd));
                println!("disconnected {} {}", addr, guid);
            }
        }
    }
}

#[actix_rt::main]
async fn main() {
    let local_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let motd = "MCPE;§5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned();
    let server_guid = 114514;
    let socket = tokio::net::UdpSocket::bind(local_addr).await.unwrap();
    let _handler = Raknet::create(|ctx| {
        let rak_server = RakServer::new(socket, server_guid, motd, ctx.address(), 8);
        Raknet { rak_server }
    });
    actix_rt::Arbiter::local_join().await;
}
