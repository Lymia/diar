use crate::errors::*;
use fastcdc::v2020::FastCDC;
use std::fmt::{Display, Formatter};
use std::hash::Hasher;
use std::io::Read;

const HASH_COUNT: usize = 64;

fn bcj_arm_code(buf: &mut [u8]) -> usize {
    let len = buf.len();
    if len < 4 {
        return 0;
    }
    let end = len - 4;
    let mut i = 0;
    while i <= end {
        let b3 = buf[i + 3];

        if b3 == 0xEB {
            let dest = 0; // modified: we are trying to remove bcj material
            buf[i + 2] = ((dest >> 16) & 0xff) as u8;
            buf[i + 1] = ((dest >> 8) & 0xff) as u8;
            buf[i] = (dest & 0xff) as u8;
        }
        i += 4;
    }
    i
}

fn bcj_arm_thumb_code(buf: &mut [u8]) -> usize {
    let len = buf.len();
    if len < 4 {
        return 0;
    }
    let end = len - 4;

    let mut i = 0;
    while i <= end {
        let b1 = buf[i + 1] as i32;
        let b3 = buf[i + 3] as i32;

        if (b3 & 0xF8) == 0xF8 && (b1 & 0xF8) == 0xF0 {
            let dest = 0; // modified: we are trying to remove bcj material
            buf[i + 1] = (0xF0 | ((dest >> 19) & 0x07)) as u8;
            buf[i] = (dest >> 11) as u8;
            buf[i + 3] = (0xf8 | ((dest >> 8) & 0x07)) as u8;
            buf[i + 2] = ((dest) & 0xff) as u8;
            i += 2;
        }
        i += 2;
    }

    i
}

#[derive(Copy, Clone, Debug, Hash)]
pub struct ContentHash {
    data: [u64; HASH_COUNT],
}
impl ContentHash {
    pub fn calculate(mut stream: impl Read) -> Result<Self> {
        // read the rom
        let mut data = Vec::new();
        stream.read_to_end(&mut data)?;

        // bcj filter the data
        bcj_arm_code(&mut data);
        bcj_arm_thumb_code(&mut data);

        // calculate the cdc chunking
        let mut cdc = FastCDC::new(&data, 1024, 1024 * 16, 1024 * 32);
        let mut hash_data = [0; HASH_COUNT];
        for chunk in cdc {
            let block = &data[chunk.offset..chunk.offset + chunk.length];

            // skip highly entropic blocks
            let ent = entropy::shannon_entropy(block);
            if ent > 7.0 {
                continue;
            }

            // bcj filter and rehash
            let mut hasher = twox_hash::Xxh3Hash64::default();
            hasher.write(block);
            let hash = hasher.finish();

            // add a bit to the data
            hash_data[(hash as usize) % HASH_COUNT] ^= 1 << ((hash >> 32) % 64);
        }

        Ok(ContentHash { data: hash_data })
    }

    pub fn distance(&self, other: &ContentHash) -> u32 {
        let mut differences = 0;
        for i in 0..HASH_COUNT {
            differences += (self.data[i] ^ other.data[i]).count_ones();
        }
        differences
    }
}
impl Display for ContentHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for i in 0..HASH_COUNT {
            write!(f, "{:016x}", self.data[i])?;
        }
        Ok(())
    }
}
