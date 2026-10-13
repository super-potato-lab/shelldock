use serde::Serialize;
use std::{
    collections::HashMap,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const COMMAND: &str = r#"LC_ALL=C; export LC_ALL; printf '__SD_SYS__\n'; uname -sr; printf '__SD_CPU__\n'; head -n 1 /proc/stat; printf '__SD_MEM__\n'; cat /proc/meminfo; printf '__SD_LOAD__\n'; cat /proc/loadavg; printf '__SD_UPTIME__\n'; cat /proc/uptime; printf '__SD_NET__\n'; cat /proc/net/dev; printf '__SD_DISK__\n'; df -Pk; printf '__SD_PS__\n'; ps -eo pid,user,pcpu,pmem,comm --sort=-pcpu | head -n 13"#;

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub system: String,
    pub timestamp: u64,
    pub uptime: f64,
    pub cpu_percent: Option<f64>,
    pub memory_total: u64,
    pub memory_available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub load: Vec<f64>,
    pub interfaces: Vec<Interface>,
    pub disks: Vec<Disk>,
    pub processes: Vec<Process>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    pub name: String,
    pub received: u64,
    pub sent: u64,
    pub receive_rate: Option<f64>,
    pub send_rate: Option<f64>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub available: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    pub pid: u32,
    pub user: String,
    pub cpu: f64,
    pub memory: f64,
    pub command: String,
}

#[derive(Default)]
pub struct Tracker {
    previous: Option<Counters>,
}
struct Counters {
    at: Instant,
    total: u64,
    idle: u64,
    net: HashMap<String, (u64, u64)>,
}

impl Tracker {
    pub fn sample(&mut self, text: &str) -> anyhow::Result<Metrics> {
        self.sample_at(text, Instant::now())
    }
    fn sample_at(&mut self, text: &str, now: Instant) -> anyhow::Result<Metrics> {
        let mut sections: HashMap<String, String> = HashMap::new();
        let mut section = String::new();
        for line in text.lines() {
            if line.starts_with("__SD_") && line.ends_with("__") {
                section = line.to_string();
            } else {
                sections
                    .entry(section.clone())
                    .or_default()
                    .push_str(&format!("{line}\n"));
            }
        }
        let cpu: Vec<u64> = sections
            .get("__SD_CPU__")
            .map(|s| {
                s.split_whitespace()
                    .skip(1)
                    .take(8)
                    .filter_map(|x| x.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        anyhow::ensure!(cpu.len() >= 4, "此主机未提供有效 Linux /proc 监控数据");
        let total: u64 = cpu.iter().sum();
        let idle = cpu[3] + cpu.get(4).copied().unwrap_or(0);
        let mut m = Metrics {
            system: sections
                .get("__SD_SYS__")
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ..Default::default()
        };
        let elapsed = self
            .previous
            .as_ref()
            .map(|p| now.duration_since(p.at).as_secs_f64())
            .unwrap_or(0.0);
        if let Some(p) = &self.previous {
            if let (Some(dt), Some(di)) = (total.checked_sub(p.total), idle.checked_sub(p.idle)) {
                if dt > 0 && di <= dt {
                    m.cpu_percent = Some(((dt - di) as f64 / dt as f64 * 100.0).clamp(0.0, 100.0));
                }
            }
        }
        let mem: HashMap<&str, u64> = sections
            .get("__SD_MEM__")
            .map(|s| {
                s.lines()
                    .filter_map(|l| {
                        let mut f = l.split_whitespace();
                        Some((
                            f.next()?.trim_end_matches(':'),
                            f.next()?.parse::<u64>().ok()?.saturating_mul(1024),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        m.memory_total = *mem.get("MemTotal").unwrap_or(&0);
        m.memory_available = mem
            .get("MemAvailable")
            .copied()
            .unwrap_or_else(|| {
                mem.get("MemFree").copied().unwrap_or(0)
                    + mem.get("Buffers").copied().unwrap_or(0)
                    + mem.get("Cached").copied().unwrap_or(0)
            })
            .min(m.memory_total);
        m.swap_total = *mem.get("SwapTotal").unwrap_or(&0);
        m.swap_free = *mem.get("SwapFree").unwrap_or(&0);
        m.load = sections
            .get("__SD_LOAD__")
            .map(|s| {
                s.split_whitespace()
                    .take(3)
                    .filter_map(|x| x.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        m.uptime = sections
            .get("__SD_UPTIME__")
            .and_then(|s| s.split_whitespace().next()?.parse().ok())
            .unwrap_or(0.0);
        let mut net = HashMap::new();
        if let Some(s) = sections.get("__SD_NET__") {
            for l in s.lines() {
                if let Some((name, rest)) = l.split_once(':') {
                    let name = name.trim().to_string();
                    let v: Vec<u64> = rest
                        .split_whitespace()
                        .filter_map(|x| x.parse().ok())
                        .collect();
                    if v.len() < 9 {
                        continue;
                    }
                    let (rx, tx) = (v[0], v[8]);
                    let old = self.previous.as_ref().and_then(|p| p.net.get(&name));
                    let rates = old.and_then(|(a, b)| {
                        if elapsed > 0.0 {
                            Some((
                                rx.checked_sub(*a)? as f64 / elapsed,
                                tx.checked_sub(*b)? as f64 / elapsed,
                            ))
                        } else {
                            None
                        }
                    });
                    net.insert(name.clone(), (rx, tx));
                    m.interfaces.push(Interface {
                        name,
                        received: rx,
                        sent: tx,
                        receive_rate: rates.map(|r| r.0),
                        send_rate: rates.map(|r| r.1),
                    });
                }
            }
        }
        if let Some(s) = sections.get("__SD_DISK__") {
            for l in s.lines().skip(1) {
                let f: Vec<&str> = l.split_whitespace().collect();
                if f.len() >= 6 {
                    if let (Ok(t), Ok(u), Ok(a)) = (
                        f[1].parse::<u64>(),
                        f[2].parse::<u64>(),
                        f[3].parse::<u64>(),
                    ) {
                        m.disks.push(Disk {
                            name: f[0].into(),
                            total: t * 1024,
                            used: u * 1024,
                            available: a * 1024,
                            mount: f[5..].join(" "),
                        });
                    }
                }
            }
        }
        if let Some(s) = sections.get("__SD_PS__") {
            for l in s.lines().skip(1) {
                let f: Vec<&str> = l.split_whitespace().collect();
                if f.len() >= 5 {
                    if let (Ok(pid), Ok(cpu), Ok(memory)) =
                        (f[0].parse(), f[2].parse(), f[3].parse())
                    {
                        m.processes.push(Process {
                            pid,
                            user: f[1].into(),
                            cpu,
                            memory,
                            command: f[4..].join(" "),
                        });
                    }
                }
            }
        }
        self.previous = Some(Counters {
            at: now,
            total,
            idle,
            net,
        });
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn data(cpu: &str, rx: u64) -> String {
        format!("__SD_SYS__\nLinux test\n__SD_CPU__\ncpu {cpu}\n__SD_MEM__\nMemTotal: 1000 kB\nMemAvailable: 400 kB\n__SD_NET__\neth0: {rx} 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n")
    }
    #[test]
    fn derives_rates_and_handles_counter_reset() {
        let mut t = Tracker::default();
        let at = Instant::now();
        let a = t
            .sample_at(&data("100 0 50 800 50 0 0 0 999 0", 100), at)
            .unwrap();
        assert!(a.cpu_percent.is_none());
        let b = t
            .sample_at(
                &data("120 0 60 850 70 0 0 0 9999 0", 500),
                at + Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(b.cpu_percent, Some(30.0));
        assert_eq!(b.interfaces[0].receive_rate, Some(200.0));
        assert_eq!(b.memory_available, 409600);
        let c = t
            .sample_at(&data("1 0 0 1 0 0 0 0", 1), at + Duration::from_secs(4))
            .unwrap();
        assert!(c.cpu_percent.is_none());
        assert!(c.interfaces[0].receive_rate.is_none());
    }
    #[test]
    fn unavailable_is_not_reported_as_zero() {
        assert!(Tracker::default().sample("/proc/stat: not found").is_err());
    }
}
