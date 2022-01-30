use crate::packets::MAGIC;
use crate::reader::Endian;
use byteorder::*;
use bytes::{BufMut, BytesMut};
use std::{
    net::{IpAddr, SocketAddr},
    str,
};

pub struct Writer<'a> {
    inner: &'a mut BytesMut,
}

impl<'a> Writer<'a> {
    pub fn new(inner: &'a mut BytesMut) -> Self {
        Self { inner }
    }
    pub fn write(&mut self, v: &[u8]) {
        self.inner.put(v);
    }

    pub fn write_u8(&mut self, v: u8) {
        self.inner.put_u8(v);
    }

    pub fn write_u16(&mut self, v: u16, n: Endian) {
        match n {
            Endian::Big => self.inner.put_u16(v),
            Endian::Little => self.inner.put_u16_le(v),
        }
    }
    pub fn write_u32(&mut self, v: u32, n: Endian) {
        match n {
            Endian::Big => self.inner.put_u32(v),
            Endian::Little => self.inner.put_u32_le(v),
        }
    }
    pub fn write_u24(&mut self, v: u32, n: Endian) {
        let mut buf = [0; 3];
        match n {
            Endian::Big => BigEndian::write_u24(&mut buf, v),
            Endian::Little => LittleEndian::write_u24(&mut buf, v),
        }
        self.write(&buf);
    }

    pub fn write_u64(&mut self, v: u64, n: Endian) {
        match n {
            Endian::Big => self.inner.put_u64(v),
            Endian::Little => self.inner.put_u64_le(v),
        }
    }

    pub fn write_i64(&mut self, v: i64, n: Endian) {
        match n {
            Endian::Big => self.inner.put_i64(v),
            Endian::Little => self.inner.put_i64_le(v),
        }
    }

    pub fn write_string(&mut self, body: &str) {
        let raw = body.as_bytes();
        self.write_u16(raw.len() as u16, Endian::Big);
        self.write(raw)
    }
    pub fn write_magic(&mut self) {
        self.write(&MAGIC)
    }
    pub fn write_address(&mut self, address: SocketAddr) {
        if address.is_ipv4() {
            self.write_u8(0x4);
            let ip_bytes = match address.ip() {
                IpAddr::V4(ip) => ip.octets().to_vec(),
                _ => vec![0; 4],
            };

            self.write_u8(0xff - ip_bytes[0]);
            self.write_u8(0xff - ip_bytes[1]);
            self.write_u8(0xff - ip_bytes[2]);
            self.write_u8(0xff - ip_bytes[3]);
            self.write_u16(address.port(), Endian::Big)
        } else {
            self.write_u16(23, Endian::Little);
            self.write_u16(address.port(), Endian::Big);
            self.write_u32(0, Endian::Big);
            let ip_bytes = match address.ip() {
                IpAddr::V6(ip) => ip.octets().to_vec(),
                _ => vec![0; 16],
            };
            self.write(&ip_bytes);
            self.write_u32(0, Endian::Big)
        }
    }
    pub fn pos(&self) -> usize {
        self.inner.len()
    }
}
