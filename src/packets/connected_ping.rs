use crate::{
    packets::Packet,
    reader::{Endian, Reader},
    writer::Writer,
};
use bytes::BytesMut;
use std::io::Result;

#[derive(Clone)]
pub struct ConnectedPing {
    pub client_timestamp: i64,
}

impl ConnectedPing {
    pub fn new(time: i64) -> Self {
        Self {
            client_timestamp: time,
        }
    }
}

impl Packet for ConnectedPing {
    const ID: u8 = 0x0;
    fn read(payload: &[u8]) -> Result<Self> {
        let mut cursor = Reader::new(payload);
        Ok(Self {
            client_timestamp: cursor.read_i64(Endian::Big)?,
        })
    }
    fn write(&self, bytes: &mut BytesMut) {
        let mut cursor = Writer::new(bytes);
        cursor.write_i64(self.client_timestamp, Endian::Big);
    }
}
