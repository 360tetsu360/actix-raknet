use bytes::BytesMut;

use crate::packets::Packet;
use crate::reader::{Endian, Reader};
use crate::writer::Writer;
use std::{io::Result, net::SocketAddr};

#[derive(Clone)]
pub struct NewIncomingConnection {
    pub server_address: SocketAddr,
    pub request_timestamp: i64,
    pub accepted_timestamp: i64,
}

impl Packet for NewIncomingConnection {
    const ID: u8 = 0x13;
    fn read(payload: &[u8]) -> Result<Self> {
        let mut cursor = Reader::new(payload);
        Ok(Self {
            server_address: cursor.read_address()?,
            request_timestamp: {
                cursor.next(((payload.len() - 16) - cursor.pos() as usize) as u64);
                cursor.read_i64(Endian::Big)?
            },
            accepted_timestamp: cursor.read_i64(Endian::Big)?,
        })
    }
    fn write(&self, bytes: &mut BytesMut) {
        let mut cursor = Writer::new(bytes);
        cursor.write_address(self.server_address);
        cursor.write(&[0x6u8; 10]);
        cursor.write_i64(self.request_timestamp, Endian::Big);
        cursor.write_i64(self.accepted_timestamp, Endian::Big);
    }
}
