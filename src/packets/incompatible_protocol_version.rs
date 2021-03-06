use bytes::BytesMut;

use crate::packets::Packet;
use crate::reader::{Endian, Reader};
use crate::writer::Writer;
use std::io::Result;

#[derive(Clone)]
pub struct IncompatibleProtocolVersion {
    pub server_protocol: u8,
    _magic: bool,
    pub server_guid: u64,
}

impl IncompatibleProtocolVersion {
    pub fn new(protocol_v: u8, guid: u64) -> Self {
        Self {
            server_protocol: protocol_v,
            _magic: true,
            server_guid: guid,
        }
    }
}

impl Packet for IncompatibleProtocolVersion {
    const ID: u8 = 0x19;
    fn read(payload: &[u8]) -> Result<Self> {
        let mut cursor = Reader::new(payload);
        Ok(Self {
            server_protocol: cursor.read_u8()?,
            _magic: cursor.read_magic()?,
            server_guid: cursor.read_u64(Endian::Big)?,
        })
    }
    fn write(&self, bytes: &mut BytesMut) {
        let mut cursor = Writer::new(bytes);
        cursor.write_u8(self.server_protocol);
        cursor.write_magic();
        cursor.write_u64(self.server_guid, Endian::Big);
    }
}
