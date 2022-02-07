use std::{
    collections::{BTreeSet, HashMap},
    io::Result,
};

use bytes::BytesMut;

use crate::packets::{frame::Frame, Reliability};

pub(crate) struct ACKQueue {
    lowest: u32,
    highest: u32,
    store_seq: BTreeSet<u32>,
}

impl ACKQueue {
    pub fn new() -> Self {
        Self {
            lowest: 0,
            highest: 0,
            store_seq: BTreeSet::new(),
        }
    }

    pub fn add(&mut self, sequence: u32) {
        if self.lowest <= sequence && !self.store_seq.contains(&sequence) {
            if self.highest <= sequence {
                self.highest = sequence + 1;
            }
            self.store_seq.insert(sequence);
        }
    }

    pub fn clear(&mut self) -> (Vec<(u32, u32)>, Vec<u32>) {
        let mut received: Vec<(u32, u32)> = vec![];
        let mut missing = vec![];
        for i in self.lowest..self.highest {
            if let Some(num) = self.store_seq.take(&i) {
                if received.is_empty() {
                    received.push((num, num));
                } else if received.last().unwrap().1 + 1 == num {
                    received.last_mut().unwrap().1 = num;
                } else {
                    received.push((num, num));
                }
            } else {
                missing.push(i);
            }
        }
        self.lowest = self.highest;
        (received, missing)
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

    let (received, missing) = y.clear();
    dbg!(&received);
    assert_eq!(received, vec![(0, 9), (11, 19)]);
    dbg!(&missing);
    assert_eq!(missing, vec![10]);
}
