use std::env;

#[cfg(target_os = "linux")]
use std::fs;

#[derive(Debug, Clone, Copy)]
pub struct SystemResources {
    pub free_vram_mb: u64,
    pub cpu_usage_pct: f32,
}

pub fn get_current_resources() -> SystemResources {
    SystemResources {
        free_vram_mb: configured_free_vram_mb().unwrap_or(8_000),
        cpu_usage_pct: current_cpu_usage_pct().unwrap_or(0.0),
    }
}

pub fn has_local_resource_pressure(complexity_score: u8, resources: &SystemResources) -> bool {
    complexity_score >= 4 || ((2..=3).contains(&complexity_score) && resources.free_vram_mb < 3_000)
}

fn configured_free_vram_mb() -> Option<u64> {
    env::var("AURAROUTE_FREE_VRAM_MB")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
}

#[cfg(target_os = "linux")]
fn current_cpu_usage_pct() -> Option<f32> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let first_line = stat.lines().next()?;
    let mut fields = first_line.split_whitespace();

    if fields.next()? != "cpu" {
        return None;
    }

    let values = fields
        .filter_map(|field| field.parse::<u64>().ok())
        .collect::<Vec<_>>();

    if values.len() < 4 {
        return None;
    }

    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let total = values.iter().copied().sum::<u64>();

    if total == 0 {
        return None;
    }

    let busy = total.saturating_sub(idle);
    Some((busy as f32 / total as f32) * 100.0)
}

#[cfg(not(target_os = "linux"))]
fn current_cpu_usage_pct() -> Option<f32> {
    env::var("AURAROUTE_CPU_USAGE_PCT")
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
}
