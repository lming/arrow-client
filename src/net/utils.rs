// Copyright 2015 click2stream, Inc.
// 
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// 
//     http://www.apache.org/licenses/LICENSE-2.0
// 
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Common networking utils.

use std::io;
use std::ptr;

use std::io::Write;
use std::net::{SocketAddr, IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

use utils::RuntimeError;

use time;

/// Get socket address from a given argument.
pub fn get_socket_address<T>(s: T) -> Result<SocketAddr, RuntimeError>
    where T: ToSocketAddrs {
    let mut addrs = try!(s.to_socket_addrs()
        .or(Err(RuntimeError::from("unable get socket address"))));
    
    match addrs.next() {
        Some(addr) => Ok(addr),
        _          => Err(RuntimeError::from("unable get socket address"))
    }
}

/// Timeout provider for various network protocols.
#[derive(Debug)]
pub struct Timeout {
    timeout: Option<u64>,
}

impl Timeout {
    /// Create a new instance of Timeout. The initial state is reset.
    pub fn new() -> Timeout {
        Timeout {
            timeout: None
        }
    }
    
    /// Clear the timeout (i.e. the check() method will always return true 
    /// until the timeout is set).
    pub fn clear(&mut self) -> &mut Self {
        self.timeout = None;
        self
    }
    
    /// Set the timeout. 
    ///
    /// The timeout will expire after a specified delay in miliseconds.
    pub fn set(&mut self, delay_ms: u64) -> &mut Self {
        self.timeout = Some(time::precise_time_ns() + delay_ms * 1000000);
        self
    }
    
    /// Check if the timeout has already expired.
    ///
    /// The method returns false if the timeout has already expired, otherwise 
    /// true is returned.
    pub fn check(&self) -> bool {
        match self.timeout {
            Some(t) => time::precise_time_ns() <= t,
            None    => true
        }
    }
}

/// Writer that can be used for buffering data.
pub struct WriteBuffer {
    buffer:   Vec<u8>,
    capacity: usize,
    offset:   usize,
    used:     usize,
}

impl WriteBuffer {
    /// Create a new buffer with a given capacity. Note that the capacity is 
    /// only a soft limit. The buffer will always allow you to write more than 
    /// its capacity.
    pub fn new(capacity: usize) -> WriteBuffer {
        let mut res = WriteBuffer {
            buffer:   Vec::with_capacity(capacity),
            capacity: capacity,
            offset:   0,
            used:     0
        };
        
        // TODO: replace this with resize (after it's stabilized)
        let buf_capacity = res.buffer.capacity();
        unsafe {
            res.buffer.set_len(buf_capacity);
        }
        
        res
    }
    
    /// Check if the buffer is full.
    pub fn is_full(&self) -> bool {
        self.used >= self.capacity
    }
    
    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.used == 0
    }
    
    /// Get number of bytes available until the soft limit is reached.
    pub fn available(&self) -> usize {
        if self.is_full() {
            0
        } else {
            self.capacity - self.used
        }
    }
    
    /// Get number of buffered bytes.
    pub fn buffered(&self) -> usize {
        self.used
    }
    
    /// Get slice of bytes of the currently buffered data.
    pub fn as_bytes(&self) -> &[u8] {
        let start = self.offset;
        let end   = start + self.used;
        &self.buffer[start..end]
    }
    
    /// Drop a given number of bytes from the buffer.
    pub fn drop(&mut self, count: usize) {
        if count > self.used {
            self.offset += self.used;
            self.used    = 0;
        } else {
            self.offset += count;
            self.used   -= count;
        }
    }
    
    /// Drop all buffered data.
    pub fn clear(&mut self) {
        self.offset += self.used;
        self.used    = 0;
    }
}

impl Write for WriteBuffer {
    /// Write given data into the buffer.
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        // expand buffer if needed
        let buf_capacity = self.buffer.capacity();
        if (self.used + data.len()) > buf_capacity {
            // TODO: replace this with resize (after it's stabilized)
            self.buffer.reserve(self.used + data.len() - buf_capacity);
            let buf_capacity = self.buffer.capacity();
            unsafe {
                self.buffer.set_len(buf_capacity);
            }
        }
        
        // shift the buffered data to the left if needed
        let buf_capacity = self.buffer.capacity();
        if (self.offset + self.used + data.len()) > buf_capacity {
            let dst = self.buffer.as_mut_ptr();
            unsafe {
                let src = dst.offset(self.offset as isize);
                ptr::copy(src, dst, self.used); 
            }
            self.offset = 0;
        }
        
        // write given data
        let offset     = self.offset + self.used;
        let mut buffer = &mut self.buffer[offset..];
        buffer.write_all(data)
            .unwrap();
        
        self.used += data.len();
        
        Ok(data.len())
    }
    
    /// Do nothing.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// IpAddr extension.
pub trait IpAddrEx {
    /// Get left-aligned byte representation of the IP address.
    fn bytes(&self) -> [u8; 16];
    
    /// Get IP address version.
    fn version(&self) -> u8;
}

impl IpAddrEx for IpAddr {
    fn bytes(&self) -> [u8; 16] {
        match self {
            &IpAddr::V4(ref ip_addr) => ip_addr.bytes(),
            &IpAddr::V6(ref ip_addr) => ip_addr.bytes()
        }
    }
    
    fn version(&self) -> u8 {
        match self {
            &IpAddr::V4(ref ip_addr) => ip_addr.version(),
            &IpAddr::V6(ref ip_addr) => ip_addr.version()
        }
    }
}

impl IpAddrEx for Ipv4Addr {
    fn bytes(&self) -> [u8; 16] {
        let octets  = self.octets();
        let mut res = [0u8; 16];
        
        for i in 0..octets.len() {
            res[i] = octets[i];
        }
        
        res
    }
    
    fn version(&self) -> u8 {
        4
    }
}

impl IpAddrEx for Ipv6Addr {
    fn bytes(&self) -> [u8; 16] {
        let segments = self.segments();
        let mut res  = [0u8; 16];
        
        for i in 0..segments.len() {
            let segment = segments[i];
            let j       = i << 1;
            res[j]      = (segment >> 8) as u8;
            res[j + 1]  = (segment & 0xff) as u8;
        }
        
        res
    }
    
    fn version(&self) -> u8 {
        6
    }
}
