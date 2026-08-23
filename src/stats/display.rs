// SPDX-License-Identifier: AGPL-3.0-only

use std::{collections::VecDeque, fmt::Write};
use super::{GIB, HISTORY_LEN, KIB, MIB};

const GRAPH_WIDTH: f64 = 520.0;
const GRAPH_HEIGHT: f64 = 72.0;
const GRAPH_PADDING: f64 = 3.0;

pub(super) fn push_optional(history: &mut VecDeque<f64>, value: Option<f64>) {
    let Some(value) = value.filter(|v| v.is_finite()) else { return };
    history.push_back(value.max(0.0));
    while history.len() > HISTORY_LEN { history.pop_front(); }
}

pub(super) fn line_svg_fixed(samples: &VecDeque<f64>, max: f64) -> String {
    line_svg(samples, 0.0, max.max(1.0))
}

pub(super) fn line_svg_auto(samples: &VecDeque<f64>) -> String {
    if samples.is_empty() { return empty_svg(); }
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for value in samples {
        min = min.min(*value);
        max = max.max(*value);
    }
    if (max - min).abs() < f64::EPSILON {
        min = 0.0;
        max = max.max(1.0);
    } else {
        let margin = (max - min) * 0.12;
        min = (min - margin).max(0.0);
        max += margin;
    }
    line_svg(samples, min, max)
}

fn line_svg(samples: &VecDeque<f64>, min: f64, max: f64) -> String {
    if samples.is_empty() || max <= min { return empty_svg(); }

    let start = samples.len().saturating_sub(HISTORY_LEN);
    let values: Vec<f64> = samples.iter().skip(start).copied().collect();
    let drawable_w = GRAPH_WIDTH - 2.0 * GRAPH_PADDING;
    let drawable_h = GRAPH_HEIGHT - 2.0 * GRAPH_PADDING;
    let step_x = if values.len() > 1 { drawable_w / (values.len() - 1) as f64 } else { 0.0 };

    let mut points = String::with_capacity(values.len() * 14);
    for (index, value) in values.iter().enumerate() {
        let x = GRAPH_PADDING + index as f64 * step_x;
        let normalized = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let y = GRAPH_PADDING + (1.0 - normalized) * drawable_h;
        if index > 0 { points.push(' '); }
        let _ = write!(&mut points, "{x:.1},{y:.1}");
    }

    let last_value = *values.last().unwrap_or(&min);
    let last_normalized = ((last_value - min) / (max - min)).clamp(0.0, 1.0);
    let last_x = GRAPH_PADDING + (values.len().saturating_sub(1)) as f64 * step_x;
    let last_y = GRAPH_PADDING + (1.0 - last_normalized) * drawable_h;

    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {GRAPH_WIDTH} {GRAPH_HEIGHT}" preserveAspectRatio="none">
<g fill="none" stroke="currentColor">
  <path d="M3 18 H517 M3 36 H517 M3 54 H517" opacity="0.14" stroke-width="1"/>
  <polyline points="{points}" opacity="0.22" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
  <polyline points="{points}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
</g>
<circle cx="{last_x:.1}" cy="{last_y:.1}" r="2.6" fill="currentColor"/>
</svg>"#)
}

fn empty_svg() -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {GRAPH_WIDTH} {GRAPH_HEIGHT}" preserveAspectRatio="none">
<path d="M3 36 H517" fill="none" stroke="currentColor" opacity="0.18" stroke-width="1"/>
</svg>"#)
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
mod tests {
    use super::*;
    #[test]
    fn svg_contains_points() {
        let samples=VecDeque::from(vec![0.0,25.0,50.0,75.0,100.0]);
        let svg=line_svg_fixed(&samples,100.0);
        assert!(svg.contains("polyline"));
        assert!(svg.contains("circle"));
    }
}
