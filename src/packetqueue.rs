use std::collections::HashMap;

use actix::clock::Instant;

use crate::packets::{frame::Frame, frame_set::FrameSet};

const NEEDS_B_AND_AS_FLAG: u8 = 0x4;

const CONTINUOUS_SEND_FLAG: u8 = 0x8;

pub struct PacketQueue {
    pub queue: HashMap<u32, FrameSet>,
    pub time_passed: HashMap<u32, (Instant, bool)>,
    pub max: u32,
    send_min: u32,
    resend: Vec<u32>,
    set_size: usize,
    set_queue: Vec<Frame>,
    mtu: u16,
}

impl PacketQueue {
    pub fn new(mtu: u16) -> Self {
        Self {
            queue: HashMap::new(),
            time_passed: HashMap::new(),
            max: 0,
            send_min: 0,
            resend: vec![],
            set_size: 0,
            set_queue: vec![],
            mtu,
        }
    }
    pub fn add_frame(&mut self, frame: Frame) {
        if self.set_size + frame.length() < (self.mtu - 42) as usize && !frame.split {
            self.set_size += frame.length();
            self.set_queue.push(frame);
        } else {
            let set = FrameSet {
                header: {
                    if frame.split {
                        0x80 | NEEDS_B_AND_AS_FLAG | CONTINUOUS_SEND_FLAG
                    } else {
                        0x80 | NEEDS_B_AND_AS_FLAG
                    }
                },
                sequence_number: self.max,
                datas: vec![frame],
            };
            self.add(set);
        }
    }
    pub fn add(&mut self, frame_set: FrameSet) {
        if frame_set.sequence_number == self.max {
            self.max += 1;
            self.time_passed
                .insert(frame_set.sequence_number, (Instant::now(), false));
            self.queue.insert(frame_set.sequence_number, frame_set);
        }
    }
    pub fn received(&mut self, sequence: u32) {
        if self.queue.contains_key(&sequence) {
            self.queue.remove(&sequence);
            self.time_passed.remove(&sequence);
        }
    }
    pub fn tick(&mut self) {
        if !self.set_queue.is_empty() {
            let set = FrameSet {
                header: 0x80 | NEEDS_B_AND_AS_FLAG,
                sequence_number: self.max,
                datas: self.set_queue.clone(),
            };
            self.add(set);
            self.set_queue.clear();
            self.set_size = 0;
        }
        let time_passed = Instant::now();
        for elem in self.time_passed.iter_mut() {
            if elem.1 .1 && time_passed.duration_since(elem.1 .0).as_millis() > 1000 {
                self.resend.push(*elem.0)
            }
        }
    }
    pub fn readd(&mut self) {
        for resend in self.resend.iter() {
            if self.queue.contains_key(resend) {
                let mut added = self.queue.get_mut(resend).unwrap().clone();
                added.sequence_number = self.max;
                self.queue.insert(self.max, added);
                self.queue.remove(resend);
                self.time_passed.remove(resend);
                self.time_passed.insert(self.max, (Instant::now(), false));
                self.max += 1;
            }
        }
        self.resend.clear();
    }
    pub fn resend(&mut self, index: u32) {
        if self.time_passed.contains_key(&index) {
            let mut added = self.queue.get_mut(&index).unwrap().clone();
            added.sequence_number = self.max;
            self.queue.insert(self.max, added);
            self.queue.remove(&index);
            self.time_passed.remove(&index);
            self.time_passed.insert(self.max, (Instant::now(), false));
            self.max += 1;
        }
    }
    pub fn get_packet(&mut self) -> Vec<FrameSet> {
        //get send able packets and start timer
        self.tick();
        self.readd();
        let mut ret = vec![];
        for i in self.send_min..self.max {
            ret.push((*self.queue.get(&i).unwrap()).clone());
            self.time_passed.get_mut(&i).unwrap().1 = true;
        }
        self.send_min = self.max;
        ret
    }
}
