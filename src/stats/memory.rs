// SPDX-License-Identifier: AGPL-3.0-only

use std::fs;
use super::KIB;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct MemorySample {
    pub(super) ram_used_bytes: u64,
    pub(super) ram_total_bytes: u64,
    pub(super) swap_used_bytes: u64,
    pub(super) swap_total_bytes: u64,
}

pub(super) fn read_memory() -> Option<MemorySample> {
    parse_memory(&fs::read_to_string("/proc/meminfo").ok()?)
}

fn parse_memory(contents: &str) -> Option<MemorySample> {
    let (mut total, mut avail, mut swap_total, mut swap_free) = (None, None, Some(0u64), Some(0u64));
    for line in contents.lines() {
        let parse = |value: &str| value.split_whitespace().next()?.parse::<u64>().ok();
        if let Some(v) = line.strip_prefix("MemTotal:") { total = parse(v); }
        else if let Some(v) = line.strip_prefix("MemAvailable:") { avail = parse(v); }
        else if let Some(v) = line.strip_prefix("SwapTotal:") { swap_total = parse(v); }
        else if let Some(v) = line.strip_prefix("SwapFree:") { swap_free = parse(v); }
    }
    let ram_total_bytes = total?.saturating_mul(KIB);
    let available_bytes = avail?.saturating_mul(KIB);
    let swap_total_bytes = swap_total?.saturating_mul(KIB);
    let swap_free_bytes = swap_free?.saturating_mul(KIB);
    Some(MemorySample {
        ram_used_bytes: ram_total_bytes.saturating_sub(available_bytes),
        ram_total_bytes,
        swap_used_bytes: swap_total_bytes.saturating_sub(swap_free_bytes),
        swap_total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn meminfo() {
        let m = parse_memory("MemTotal: 1000000 kB\nMemAvailable: 250000 kB\nSwapTotal: 200000 kB\nSwapFree: 150000 kB\n").unwrap();
        assert_eq!(m.ram_used_bytes, 750000 * KIB);
        assert_eq!(m.swap_used_bytes, 50000 * KIB);
    }
}
