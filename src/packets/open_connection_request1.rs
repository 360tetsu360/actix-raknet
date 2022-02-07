use crate::packets::Packet;
use crate::reader::Reader;
use crate::writer::Writer;
use actix::prelude::*;
use bytes::BytesMut;
use std::{convert::TryInto, io::Result};

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct OpenConnectionRequest1 {
    _magic: bool,
    pub protocol_version: u8,
    pub mtu_size: u16, //[u8;mtusize]
}

impl OpenConnectionRequest1 {
    pub fn new(protocol_version: u8, mtu_size: u16) -> Self {
        Self {
            _magic: true,
            protocol_version,
            mtu_size,
        }
    }
}

impl Packet for OpenConnectionRequest1 {
    const ID: u8 = 0x5;
    fn read(payload: &[u8]) -> Result<Self> {
        let mut cursor = Reader::new(payload);
        Ok(Self {
            _magic: cursor.read_magic()?,
            protocol_version: cursor.read_u8()?,
            mtu_size: (payload.len() + 32).try_into().unwrap(),
        })
    }
    fn write(&self, bytes: &mut BytesMut) {
        let mut cursor = Writer::new(bytes);
        cursor.write_magic();
        cursor.write_u8(self.protocol_version);
        cursor.write(vec![0; (self.mtu_size as usize) - (cursor.pos() + 32)].as_slice());
    }
}
