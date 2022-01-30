use std::{collections::HashMap, io::Result};

use bytes::BytesMut;

use crate::packets::{frame::Frame, Reliability};
pub(crate) struct ACKQueue {
    pub packets: Vec<(u32, u32)>, //min max
    pub missing: Vec<u32>,
}

impl Default for ACKQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ACKQueue {
    pub fn new() -> Self {
        Self {
            packets: vec![],
            missing: vec![],
        }
    }
    pub fn add(&mut self, sequence: u32) {
        let mut added = false;
        if self.missing.contains(&sequence) {
            let index = self.missing.iter().position(|x| *x == sequence).unwrap();
            self.missing.remove(index);
        }
        for (_lowest, highest) in self.packets.iter_mut() {
            if *highest + 1 == sequence {
                *highest += 1;
                added = true;
            }
        }
        if !added {
            self.packets.sort_unstable();
            if !self.packets.is_empty() && sequence > self.packets.last().unwrap().1 {
                for num in self.packets.last().unwrap().1 + 1..sequence {
                    self.missing.push(num);
                }
            }
            self.packets.push((sequence, sequence));
        }
        self.clean();
    }
    pub fn clean(&mut self) {
        self.packets.sort_unstable();
        let clone = self.packets.clone();
        for x in 0..clone.len() {
            if x + 1 != clone.len() && clone[x].1 + 1 == clone[x + 1].0 {
                self.packets[x + 1].0 = self.packets[x].0;
                self.packets.remove(x);
            }
        }
    }

    pub fn get_send_able_and_clear(&mut self) -> Vec<(u32, u32)> {
        let ret = self.packets.clone();
        self.packets.clear();
        ret
    }
    pub fn get_missing(&self) -> Vec<u32> {
        self.missing.clone()
    }
    pub fn get_missing_len(&self) -> usize {
        self.missing.len()
    }
}

#[derive(Clone)]
pub(crate) struct SplitPacket {
    pub split_size: u32,
    pub data: HashMap<u32, Frame>,
    pub reliability: Reliability,
    pub order_index: u32,
    full: bool,
}
impl SplitPacket {
    pub fn new(split_size: u32, order_index: u32, reliability: Reliability) -> Self {
        Self {
            split_size,
            data: HashMap::new(),
            reliability,
            order_index,
            full: false,
        }
    }
    pub fn add(&mut self, index: u32, payload: Frame) {
        if index < self.split_size {
            self.data.insert(index, payload);
            if self.data.len() as u32 == self.split_size {
                self.full = true;
            }
        }
    }
    pub fn is_full(&self) -> bool {
        self.full
    }
    pub fn get_all(&mut self) -> BytesMut {
        let mut ret = BytesMut::new();
        for index in 0..self.split_size {
            ret.extend_from_slice(&self.data.get(&index).unwrap().data);
        }
        ret
    }
    pub fn get_frame(&mut self) -> Result<Frame> {
        let mut frame = Frame::new(self.reliability.clone(), self.get_all());
        frame.order_index = self.order_index;
        Ok(frame)
    }
}

pub(crate) struct SplitPacketQueue {
    pub pool: HashMap<u16, SplitPacket>,
    delete: Vec<u16>,
}
impl Default for SplitPacketQueue {
    fn default() -> Self {
        Self::new()
    }
}
impl SplitPacketQueue {
    pub fn new() -> Self {
        Self {
            pool: HashMap::new(),
            delete: vec![],
        }
    }
    pub fn add(&mut self, frame: Frame) {
        if let std::collections::hash_map::Entry::Vacant(e) = self.pool.entry(frame.split_id) {
            let mut new_split = SplitPacket::new(
                frame.split_count,
                frame.sequence_index,
                frame.reliability.clone(),
            );
            new_split.order_index = frame.order_index;
            e.insert(new_split);
        }
        self.pool
            .get_mut(&frame.split_id)
            .unwrap()
            .add(frame.split_index, frame);
    }
    pub fn get_and_clear(&mut self) -> Vec<SplitPacket> {
        let mut ret = vec![];
        for water in self.pool.iter() {
            if water.1.is_full() {
                self.delete.push(*water.0);
                ret.push(water.1.clone());
            }
        }
        for delete in self.delete.iter() {
            self.pool.remove(delete);
        }
        ret
    }
}

#[test]
fn ack_queue() {
    let mut y = ACKQueue::new();
    for x in 0..10 {
        y.add(x);
    }
    for x in 11..20 {
        y.add(x);
    }
    //y.add(10);
    let z = y.get_send_able_and_clear();
    println!("{{");
    for a in z {
        println!("  ({},{}),", a.0, a.1);
    }
    println!("}}");
    y.add(10);
    let z = y.get_send_able_and_clear();
    println!("{{");
    for a in z {
        println!("  ({},{}),", a.0, a.1);
    }
    println!("}}");
}
