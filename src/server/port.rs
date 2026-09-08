//! A loopback port that is stable per data dir (browser storage is keyed by origin), with a
//! short retry for the window in which a previous instance is still shutting down.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow};
use tokio::net::TcpListener;

const RANGE_START: u16 = 20000;
const RANGE_SIZE: u64 = 20000;

pub fn deterministic_port(data_dir: &Path) -> u16 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in data_dir.to_string_lossy().as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    RANGE_START + (hash % RANGE_SIZE) as u16
}

pub async fn bind_with_retry(addr: SocketAddr, attempts: u32, delay: Duration) -> Result<TcpListener> {
    let mut attempt = 0;
    loop {
        match TcpListener::bind(addr).await {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && attempt < attempts => {
                attempt += 1;
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(anyhow!("cannot bind {addr}: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_and_in_range() {
        let a = deterministic_port(Path::new("/home/u/.config/switcheroo"));
        assert_eq!(a, deterministic_port(Path::new("/home/u/.config/switcheroo")));
        assert!(a >= RANGE_START);
        assert_ne!(a, deterministic_port(Path::new("/tmp/x")));
    }
}
