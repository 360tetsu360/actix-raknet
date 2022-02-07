use crate::{
    packets::Packet,
    reader::{Endian, Reader},
    writer::Writer,
};
use actix::prelude::*;
use bytes::BytesMut;
use std::io::Result;

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct Ack {
    pub record_count: u16,
    pub max_equals_min: bool,
    pub sequences: (u32, u32),
}
impl Ack {
    pub fn new(sequences: (u32, u32)) -> Self {
        if sequences.0 == sequences.1 {
            Self {
                record_count: 1,
                max_equals_min: true,
                sequences,
            }
        } else {
            Self {
                record_count: 1,
                max_equals_min: false,
                sequences,
            }
        }
    }
    pub fn get_all(&self) -> Vec<u32> {
        (self.sequences.0..self.sequences.1 + 1).collect()
    }
}

impl Packet for Ack {
    const ID: u8 = 0xc0;
    fn write(&self, bytes: &mut BytesMut) {
        let mut cursor = Writer::new(bytes);
        cursor.write_u16(self.record_count, Endian::Big);
        cursor.write_u8(self.max_equals_min as u8);
        cursor.write_u24(self.sequences.0, Endian::Little);
        if !self.max_equals_min {
            cursor.write_u24(self.sequences.1, Endian::Little);
        }
    }
    fn read(payload: &[u8]) -> Result<Self> {
        let mut cursor = Reader::new(payload);
        let record_count = cursor.read_u16(Endian::Big)?;
        let max_equals_min = cursor.read_u8()? != 0;
        let sequences;
        let sequence = cursor.read_u24(Endian::Little)?;
        if max_equals_min {
            sequences = (sequence, sequence)
        } else {
            let sequence_max = cursor.read_u24(Endian::Little)?;
            sequences = (sequence, sequence_max)
        }
        Ok(Self {
            record_count,
            max_equals_min,
            sequences,
        })
    }
}
