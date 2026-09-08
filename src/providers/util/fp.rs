//! FNV-1a fingerprint of slot contents, used to skip network `whoami` calls when nothing
//! changed on disk.

pub fn fnv1a(parts: &[Option<Vec<u8>>]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut feed = |b: u8| {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    };
    for part in parts {
        match part {
            None => feed(0xff),
            Some(bytes) => {
                feed(0x01);
                for b in bytes {
                    feed(*b);
                }
            }
        }
        feed(0x00);
    }
    hash
}
