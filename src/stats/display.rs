// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::VecDeque;
use super::{GIB, HISTORY_LEN, KIB, MIB};

const SPARKLINE_WIDTH: usize = 52;

pub(super) fn push_optional(history: &mut VecDeque<f64>, value: Option<f64>) {
    let Some(value) = value.filter(|v| v.is_finite()) else { return };
    history.push_back(value.max(0.0));
    while history.len() > HISTORY_LEN { history.pop_front(); }
}

pub(super) fn sparkline_fixed(samples: &VecDeque<f64>, max: f64) -> String { sparkline(samples, 0.0, max.max(1.0)) }
pub(super) fn sparkline_auto(samples: &VecDeque<f64>) -> String {
    if samples.is_empty() { return "--".into(); }
    let (mut min, mut max)=(f64::INFINITY, f64::NEG_INFINITY);
    for v in samples { min=min.min(*v); max=max.max(*v); }
    if (max-min).abs() < f64::EPSILON { min=0.0; max=max.max(1.0); }
    sparkline(samples,min,max)
}
fn sparkline(samples: &VecDeque<f64>, min: f64, max: f64) -> String {
    const LEVELS:[char;8]=['▁','▂','▃','▄','▅','▆','▇','█'];
    if samples.is_empty() || max <= min { return "--".into(); }
    let start=samples.len().saturating_sub(SPARKLINE_WIDTH);
    samples.iter().skip(start).map(|v| {
        let n=((*v-min)/(max-min)).clamp(0.0,1.0);
        LEVELS[(n*7.0).round() as usize]
    }).collect()
}

pub(super) fn usage_bar(value:f64,width:usize)->String {
    let filled=((value.clamp(0.0,100.0)/100.0)*width as f64).round() as usize;
    format!("{}{}","█".repeat(filled),"░".repeat(width-filled))
}

pub(super) fn percent_of(used:Option<u64>,total:Option<u64>)->Option<f64> {
    match (used,total) { (Some(u),Some(t)) if t>0 => Some(u as f64*100.0/t as f64), _=>None }
}
pub(super) fn format_percent(value:Option<f64>)->String { value.filter(|v|v.is_finite()).map(|v|format!("{:>3.0}%",v.clamp(0.0,100.0))).unwrap_or_else(||" --%".into()) }
pub(super) fn format_temperature(value:Option<f64>)->String { value.filter(|v|v.is_finite()).map(|v|format!("{v:.0}°C")).unwrap_or_else(||"--°C".into()) }
pub(super) fn format_memory(label:&str,used:Option<u64>,total:Option<u64>)->String {
    match (used,total) { (Some(u),Some(t)) if t>0 => format!("{label} {:.1}/{:.1}G",u as f64/GIB,t as f64/GIB), _=>format!("{label} --/--G") }
}
pub(super) fn format_memory_long(used:Option<u64>,total:Option<u64>)->String {
    match (used,total) {
        (Some(u),Some(t)) if t>0 => format!("{:.1} / {:.1} GiB ({:.0}%)",u as f64/GIB,t as f64/GIB,u as f64*100.0/t as f64),
        (Some(_),Some(0)) => "disabled".into(), _=>"--".into()
    }
}
pub(super) fn format_rate(value:Option<f64>)->String {
    let Some(v)=value.filter(|v|v.is_finite()&&*v>=0.0) else { return "--".into() };
    if v>=GIB { format!("{:.2} GiB/s",v/GIB) }
    else if v>=MIB as f64 { format!("{:.2} MiB/s",v/MIB as f64) }
    else if v>=KIB as f64 { format!("{:.1} KiB/s",v/KIB as f64) }
    else { format!("{v:.0} B/s") }
}
pub(super) fn format_uptime(seconds:f64)->String {
    let m=(seconds.max(0.0)/60.0) as u64; let d=m/(24*60); let h=(m/60)%24; let m=m%60;
    if d>0 { format!("{d}d {h}h {m}m") } else if h>0 { format!("{h}h {m}m") } else { format!("{m}m") }
}

#[cfg(test)]
mod tests { use super::*; #[test] fn spark(){ let s=VecDeque::from(vec![0.0,25.0,50.0,75.0,100.0]); assert_eq!(sparkline_fixed(&s,100.0).chars().count(),5); } }
