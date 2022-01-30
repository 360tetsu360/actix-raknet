use std::net::SocketAddr;

use actix::prelude::*;
use actix_raknet::client::{ClientHandle, RakClient, RakClientEvent};
use bytes::BytesMut;
struct Raknet {
    rak_client: ClientHandle,
}

impl Actor for Raknet {
    type Context = Context<Self>;
}

impl Handler<RakClientEvent> for Raknet {
    type Result = ();
    fn handle(&mut self, msg: RakClientEvent, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakClientEvent::ConnectionFailed(_reason) => {}
            RakClientEvent::Connected => {
                println!("Connected");
                let a: &[u8] = &[0xfeu8; 4000];
                self.rak_client.packet(BytesMut::from(a));
            }
            RakClientEvent::Packet(p) => {
                println!("Got packet {:?}", p);
            }
            RakClientEvent::Disconnected => {
                let remote_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
                self.rak_client.connect(remote_addr);
                println!("Disconnected");
            }
        }
    }
}

impl Handler<ClientOrder> for Raknet {
    type Result = ();

    fn handle(&mut self, msg: ClientOrder, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ClientOrder::Connect(addr) => {
                self.rak_client.connect(addr);
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
enum ClientOrder {
    Connect(SocketAddr),
}

#[actix_rt::main]
async fn main() {
    let local_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let socket = tokio::net::UdpSocket::bind(local_addr).await.unwrap();
    let server_guid = 114514;
    let client = Raknet::create(|ctx| {
        let rak_client = RakClient::init(socket, server_guid, ctx.address());
        Raknet { rak_client }
    });
    let remote_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    client.do_send(ClientOrder::Connect(remote_addr));
    actix_rt::Arbiter::local_join().await;
}
