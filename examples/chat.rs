use std::{collections::HashMap, io::stdin, net::SocketAddr};

use actix::prelude::*;
use actix_raknet::{
    client::{ClientHandle, RakClient, RakClientEvent},
    server::{ConnectionHandle, RakServer, RakServerEvent},
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
            RakClientEvent::Connected => {}
            RakClientEvent::Packet(m) => {
                let str = String::from_utf8_lossy(&m);
                println!("{}", str);
            }
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

impl Handler<Packet> for Client {
    type Result = ();

    fn handle(&mut self, msg: Packet, _ctx: &mut Self::Context) -> Self::Result {
        self.rak_client.packet(msg.0);
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct Connect(SocketAddr);

#[derive(Message)]
#[rtype(result = "()")]
struct Packet(BytesMut);

struct Server {
    conns: HashMap<SocketAddr, ConnectionHandle>,
}
impl Actor for Server {
    type Context = Context<Self>;
}

impl Handler<RakServerEvent> for Server {
    type Result = ();
    fn handle(&mut self, msg: RakServerEvent, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            RakServerEvent::Connected(handle) => {
                self.conns.insert(handle.address, handle);
            }
            RakServerEvent::Packet(_, b) => {
                dbg!();
                for conn in self.conns.iter() {
                    conn.1.send(b.clone());
                }
            }
            RakServerEvent::Disconnected(addr, _guid) => {
                if self.conns.contains_key(&addr) {
                    self.conns.remove(&addr);
                }
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
        Server {
            conns: HashMap::new(),
        }
    })
}

fn server() {
    System::run(||{
        let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        block_on(create_server(0x1919, server_addr, "MCPE;§5raknet rs;390;1.17.42;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;".to_owned()));
    }).unwrap();
}

fn client() {
    System::run(|| {
        let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        let client1_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let client1 = block_on(create_client(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            client1_addr,
        ));
        client1.do_send(Connect(server_addr));

        std::thread::spawn(move || {
            let stdin = stdin();
            let mut s_buffer = String::new();

            loop {
                s_buffer.clear();
                stdin.read_line(&mut s_buffer).unwrap();
                let line = s_buffer.replace(|x| x == '\n' || x == '\r', "");
                let bytes = BytesMut::from(line.as_bytes());
                client1.do_send(Packet(bytes));
            }
        });
    })
    .unwrap();
}

fn main() {
    let stdin = stdin();

    println!("Please type in `server` or `client`.");

    let mut s = String::new();
    stdin.read_line(&mut s).unwrap();

    if s.starts_with("server") {
        println!("Starting server..");
        server()
    } else if s.starts_with("client") {
        println!("Starting client..");
        client()
    }
}
