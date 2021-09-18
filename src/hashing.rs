// This method of building a hasher was found in this
// reddit thread: https://www.reddit.com/r/rust/comments/j0fm4x/implementing_a_custom_hash_function/

use std::hash::{BuildHasher, Hasher};

// This hasher is meant to only give hashes between 0 and "positions"
// so as to implement a hash queue where there is a maximum number
// of queues that the values can be added to
pub struct BufferHasher {
    positions: u64,
    value: u64,
}

impl BufferHasher {
    pub fn new(positions: u64) -> BufferHasher {
        BufferHasher {
            positions,
            value: 0,
        }
    }
}

impl Hasher for BufferHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut message = 0 as u64;
        for byte in bytes {
            message += *byte as u64;
        }
        self.value = message % self.positions;
    }

    fn finish(&self) -> u64 {
        self.value
    }
}

pub struct BuildBufferHasher {
    positions: u64,
}

impl BuildHasher for BuildBufferHasher {
    type Hasher = BufferHasher;
    fn build_hasher(&self) -> BufferHasher {
        BufferHasher::new(self.positions)
    }
}
