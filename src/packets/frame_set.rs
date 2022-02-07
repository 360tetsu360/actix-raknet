use super::frame::Frame;
use crate::{
    reader::{Endian::Little, Reader},
    writer::Writer,
};
use actix::prelude::*;
use bytes::BytesMut;
use std::io::Result;

#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct FrameSet {
    pub header: u8,
    pub sequence_number: u32,
    pub datas: Vec<Frame>,
}

impl FrameSet {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        let size = payload.len();
        let mut cursor = Reader::new(payload);
        let mut frame_set = Self {
            header: cursor.read_u8()?,
            sequence_number: cursor.read_u24(Little)?,
            datas: vec![],
        };
        while cursor.pos() < size as u64 {
            frame_set.datas.push(Frame::decode(&mut cursor)?)
        }

        Ok(frame_set)
    }
    pub fn encode(&self) -> BytesMut {
        let mut bytes = BytesMut::new();
        let mut cursor = Writer::new(&mut bytes);
        cursor.write_u8(self.header);
        cursor.write_u24(self.sequence_number, Little);
        for frame in &self.datas {
            frame.encode(&mut bytes);
        }
        bytes
    }
}
