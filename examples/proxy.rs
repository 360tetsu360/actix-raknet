use actix::prelude::*;
use actix_raknet::{
    client::{ClientHandle, RakClient, RakClientEvent},
    server::{ConnectionHandle, RakServer, RakServerEvent},
};
use bytes::BytesMut;
use futures::executor::block_on;
use std::net::SocketAddr;

struct Client {
    rak_client: ClientHandle,
    server: Addr<Server>,
}

impl Actor for Client {
    type Context = Context<Self>;
}

impl Handler<RakClientEvent> for Client {
    type Result = ();
    fn handle(&mut self, msg: RakClientEvent, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakClientEvent::ConnectionFailed(_reason) => {}
            RakClientEvent::Connected => {
                dbg!();
            }
            RakClientEvent::Packet(p) => {
                self.server.do_send(ServerOrder::Packet(p));
            }
            RakClientEvent::Disconnected => {
                self.server.do_send(ServerOrder::Disconnect);
            }
        }
    }
}

impl Handler<ClientOrder> for Client {
    type Result = ();

    fn handle(&mut self, msg: ClientOrder, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ClientOrder::Connect(addr) => {
                self.rak_client.connect(addr);
            }
            ClientOrder::Disconnect => {
                self.rak_client.disconnect();
            }
            ClientOrder::Packet(buff) => {
                self.rak_client.packet(buff);
            }
        }
    }
}

#[derive(Message)]
#[rtype(result = "()")]
enum ClientOrder {
    Connect(SocketAddr),
    Disconnect,
    Packet(BytesMut),
}

#[derive(Message)]
#[rtype(result = "()")]
enum ServerOrder {
    Disconnect,
    Packet(BytesMut),
}

struct Server {
    _rak_server: Addr<RakServer<Self>>,
    client: Option<Addr<Client>>,
    handle: Option<ConnectionHandle>,
}

impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<RakServerEvent> for Server {
    type Result = ();
    fn handle(&mut self, msg: RakServerEvent, sctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakServerEvent::Connected(handle) => {
                dbg!();
                let local_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
                let server_guid = handle.guid;
                let socket = block_on(tokio::net::UdpSocket::bind(local_addr)).unwrap();
                self.client = Some(Client::create(|ctx| {
                    let rak_client = RakClient::init(socket, server_guid, ctx.address());
                    Client {
                        rak_client,
                        server: sctx.address(),
                    }
                }));
                self.handle = Some(handle);
                let remote_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
                self.client
                    .as_ref()
                    .unwrap()
                    .do_send(ClientOrder::Connect(remote_addr));
            }
            RakServerEvent::Packet(_handle, bytes) => {
                self.client
                    .as_ref()
                    .unwrap()
                    .do_send(ClientOrder::Packet(bytes));
            }
            RakServerEvent::Disconnected(_addr, _guid) => {
                self.client
                    .as_ref()
                    .unwrap()
                    .do_send(ClientOrder::Disconnect);
            }
        }
    }
}

impl Handler<ServerOrder> for Server {
    type Result = ();
    fn handle(&mut self, msg: ServerOrder, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ServerOrder::Disconnect => {
                self.handle.as_ref().unwrap().disconnect();
            }
            ServerOrder::Packet(p) => {
                self.handle.as_ref().unwrap().send(p);
            }
        }
    }
}

#[actix_rt::main]
async fn main() {
    let local_addr: SocketAddr = "127.0.0.1:19131".parse().unwrap();
    let motd = "MCPE;§5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned();
    let server_guid = 114514;
    let socket = tokio::net::UdpSocket::bind(local_addr).await.unwrap();
    let _handler = Server::create(|ctx| {
        let _rak_server = RakServer::new(socket, server_guid, motd, ctx.address());
        Server {
            _rak_server,
            client: None,
            handle: None,
        }
    });
    actix_rt::Arbiter::local_join().await;
}
