// SPDX-License-Identifier: AGPL-3.0-only

use std::{fs, time::Instant};

#[derive(Debug, Clone)]
pub(super) struct NetworkSample { rx_bytes: u64, tx_bytes: u64, at: Instant }

impl NetworkSample {
    pub(super) fn now(rx_bytes: u64, tx_bytes: u64) -> Self { Self { rx_bytes, tx_bytes, at: Instant::now() } }
    pub(super) fn rates_since(&self, previous: &Self) -> Option<(f64, f64)> {
        let elapsed = self.at.duration_since(previous.at).as_secs_f64();
        (elapsed > 0.0).then(|| (
            self.rx_bytes.saturating_sub(previous.rx_bytes) as f64 / elapsed,
            self.tx_bytes.saturating_sub(previous.tx_bytes) as f64 / elapsed,
        ))
    }
}

pub(super) fn read_network_totals() -> Option<(u64, u64, Vec<String>)> {
    parse_network_totals(&fs::read_to_string("/proc/net/dev").ok()?)
}

fn parse_network_totals(contents: &str) -> Option<(u64, u64, Vec<String>)> {
    let (mut rx_total, mut tx_total) = (0u64, 0u64);
    let mut interfaces = Vec::new();
    for line in contents.lines().skip(2) {
        let Some((name, values)) = line.split_once(':') else { continue };
        let name = name.trim();
        if name.is_empty() || name == "lo" { continue; }
        let fields: Vec<_> = values.split_whitespace().collect();
        if fields.len() < 16 { continue; }
        let (Some(rx), Some(tx)) = (fields[0].parse::<u64>().ok(), fields[8].parse::<u64>().ok()) else { continue };
        rx_total = rx_total.saturating_add(rx);
        tx_total = tx_total.saturating_add(tx);
        interfaces.push(name.to_string());
    }
    Some((rx_total, tx_total, interfaces))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_totals() {
        let input="Inter-| Receive | Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n lo: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n eth0: 1024 0 0 0 0 0 0 0 2048 0 0 0 0 0 0 0\n wlan0: 4096 0 0 0 0 0 0 0 8192 0 0 0 0 0 0 0\n";
        let (rx,tx,ifs)=parse_network_totals(input).unwrap();
        assert_eq!((rx,tx),(5120,10240));
        assert_eq!(ifs, vec!["eth0", "wlan0"]);
    }
}
