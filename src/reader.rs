use byteorder::*;
use std::io::Read;
use std::{
    io::{Cursor, Result},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use crate::packets::MAGIC;

pub enum Endian {
    Big,
    Little,
}

#[derive(Clone)]
pub struct Reader<'a> {
    cursor: Cursor<&'a [u8]>,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(buf),
        }
    }
    pub fn read(&mut self, buf: &mut [u8]) -> Result<()> {
        self.cursor.read_exact(buf)
    }
    pub fn read_u8(&mut self) -> Result<u8> {
        self.cursor.read_u8()
    }

    pub fn read_u16(&mut self, n: Endian) -> Result<u16> {
        match n {
            Endian::Big => self.cursor.read_u16::<BigEndian>(),
            Endian::Little => self.cursor.read_u16::<LittleEndian>(),
        }
    }

    pub fn read_u32(&mut self, n: Endian) -> Result<u32> {
        match n {
            Endian::Big => self.cursor.read_u32::<BigEndian>(),
            Endian::Little => self.cursor.read_u32::<LittleEndian>(),
        }
    }

    pub fn read_u64(&mut self, n: Endian) -> Result<u64> {
        match n {
            Endian::Big => self.cursor.read_u64::<BigEndian>(),
            Endian::Little => self.cursor.read_u64::<LittleEndian>(),
        }
    }
    pub fn read_i64(&mut self, n: Endian) -> Result<i64> {
        match n {
            Endian::Big => self.cursor.read_i64::<BigEndian>(),
            Endian::Little => self.cursor.read_i64::<LittleEndian>(),
        }
    }

    pub fn read_u24(&mut self, n: Endian) -> Result<u32> {
        match n {
            Endian::Big => self.cursor.read_u24::<BigEndian>(),
            Endian::Little => self.cursor.read_u24::<LittleEndian>(),
        }
    }

    pub fn read_string(&'a mut self) -> Result<String> {
        let size = self.read_u16(Endian::Big)?;
        let mut strbuf = vec![0u8; size.into()];
        self.read(&mut strbuf)?;
        match String::from_utf8(strbuf) {
            Ok(p) => Ok(p),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )),
        }
    }
    pub fn read_magic(&mut self) -> Result<bool> {
        let mut magic = [0; 16];
        self.cursor.read_exact(&mut magic)?;
        Ok(magic == MAGIC)
    }
    pub fn read_address(&mut self) -> Result<SocketAddr> {
        let ip_ver = self.read_u8()?;

        if ip_ver == 4 {
            let ip = Ipv4Addr::new(
                0xff - self.read_u8()?,
                0xff - self.read_u8()?,
                0xff - self.read_u8()?,
                0xff - self.read_u8()?,
            );
            let port = self.read_u16(Endian::Big)?;
            Ok(SocketAddr::new(IpAddr::V4(ip), port))
        } else {
            self.next(2);
            let port = self.read_u16(Endian::Little)?;
            self.next(4);
            let mut addr_buf = [0; 16];
            self.cursor.read_exact(&mut addr_buf)?;

            let mut address_cursor = Reader::new(&addr_buf);
            self.next(4);
            Ok(SocketAddr::new(
                IpAddr::V6(Ipv6Addr::new(
                    address_cursor.read_u16(Endian::Big)?,
                    address_cursor.read_u16(Endian::Big)?,
                    address_cursor.read_u16(Endian::Big)?,
                    address_cursor.read_u16(Endian::Big)?,
                    address_cursor.read_u16(Endian::Big)?,
                    address_cursor.read_u16(Endian::Big)?,
                    address_cursor.read_u16(Endian::Big)?,
                    address_cursor.read_u16(Endian::Big)?,
                )),
                port,
            ))
        } //IPv6 address = 128bit = u8 * 16
    }
    pub fn next(&mut self, n: u64) {
        self.cursor.set_position(self.cursor.position() + n);
    }

    pub fn pos(&self) -> u64 {
        self.cursor.position()
    }
}
