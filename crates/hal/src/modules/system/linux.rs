//! Реализация `LinuxSystemDriver` для Яндекс Станции Макс (Android 9 / Linux).

use super::{
    SystemDiskInfo, SystemHal, SystemLoadAvg, SystemMemInfo, SystemProcInfo, SystemTime,
    SystemUptime, SystemWifiInfo,
};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime as StdSystemTime, UNIX_EPOCH};

pub struct LinuxSystemDriver;

impl LinuxSystemDriver {
    pub fn open() -> Self {
        Self
    }
}

impl SystemHal for LinuxSystemDriver {
    fn time(&self) -> SystemTime {
        let dur = StdSystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let unix_sec = dur.as_secs();
        let unix_ms = dur.as_millis() as u64;

        #[cfg(unix)]
        unsafe {
            let t = unix_sec as libc::time_t;
            let mut tm: libc::tm = std::mem::zeroed();
            libc::localtime_r(&t, &mut tm);
            return SystemTime {
                unix_sec,
                unix_ms,
                year: tm.tm_year + 1900,
                month: (tm.tm_mon + 1) as u8,
                day: tm.tm_mday as u8,
                hour: tm.tm_hour as u8,
                min: tm.tm_min as u8,
                sec: tm.tm_sec as u8,
                weekday: tm.tm_wday as u8,
            };
        }

        #[cfg(not(unix))]
        {
            let rem = unix_sec % 86400;
            SystemTime {
                unix_sec,
                unix_ms,
                year: 2026,
                month: 10,
                day: 28,
                hour: ((rem / 3600) % 24) as u8,
                min: ((rem % 3600) / 60) as u8,
                sec: (rem % 60) as u8,
                weekday: 3,
            }
        }
    }

    fn uptime(&self) -> Result<SystemUptime> {
        let text = fs::read_to_string("/proc/uptime")
            .context("не удалось прочитать /proc/uptime")?;
        let mut parts = text.split_whitespace();
        let up = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let idle = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        Ok(SystemUptime {
            uptime_sec: up,
            idle_sec: idle,
        })
    }

    fn meminfo(&self) -> Result<SystemMemInfo> {
        let text = fs::read_to_string("/proc/meminfo")
            .context("не удалось прочитать /proc/meminfo")?;
        let mut total = 0u64;
        let mut free = 0u64;
        let mut avail = 0u64;
        let mut cached = 0u64;
        for line in text.lines() {
            if let Some((k, rest)) = line.split_once(':') {
                let val: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                match k.trim() {
                    "MemTotal" => total = val,
                    "MemFree" => free = val,
                    "MemAvailable" => avail = val,
                    "Cached" => cached = val,
                    _ => {}
                }
            }
        }
        let used = total.saturating_sub(avail);
        let used_pct = if total > 0 {
            (used as f64 * 100.0 / total as f64) as f32
        } else {
            0.0
        };
        Ok(SystemMemInfo {
            total_kb: total,
            free_kb: free,
            available_kb: avail,
            cached_kb: cached,
            used_pct,
        })
    }

    fn loadavg(&self) -> Result<SystemLoadAvg> {
        let text = fs::read_to_string("/proc/loadavg")
            .context("не удалось прочитать /proc/loadavg")?;
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 4 {
            let load1 = parts[0].parse().unwrap_or(0.0);
            let load5 = parts[1].parse().unwrap_or(0.0);
            let load15 = parts[2].parse().unwrap_or(0.0);
            let (running, total) = parts[3]
                .split_once('/')
                .map(|(r, t)| (r.parse().unwrap_or(1), t.parse().unwrap_or(1)))
                .unwrap_or((1, 1));
            return Ok(SystemLoadAvg {
                load1,
                load5,
                load15,
                running_threads: running,
                total_threads: total,
            });
        }
        bail!("некорректный формат /proc/loadavg")
    }

    fn disk(&self) -> Result<SystemDiskInfo> {
        #[cfg(unix)]
        unsafe {
            let mut st: libc::statvfs = std::mem::zeroed();
            let path = if Path::new("/data").exists() {
                b"/data\0".as_ptr()
            } else {
                b"/\0".as_ptr()
            };
            if libc::statvfs(path as *const libc::c_char, &mut st) == 0 {
                let bsize = st.f_frsize as u64;
                let total_mb = (st.f_blocks as u64 * bsize) / (1024 * 1024);
                let free_mb = (st.f_bfree as u64 * bsize) / (1024 * 1024);
                let avail_mb = (st.f_bavail as u64 * bsize) / (1024 * 1024);
                let used_mb = total_mb.saturating_sub(free_mb);
                let used_pct = if total_mb > 0 {
                    (used_mb as f64 * 100.0 / total_mb as f64) as f32
                } else {
                    0.0
                };
                return Ok(SystemDiskInfo {
                    total_mb,
                    free_mb,
                    available_mb: avail_mb,
                    used_pct,
                });
            }
        }
        bail!("вызов statvfs(/data) завершился ошибкой")
    }

    fn wifi(&self) -> Result<SystemWifiInfo> {
        let mut info = SystemWifiInfo {
            iface: "wlan0".into(),
            ..Default::default()
        };

        if Path::new("/system/bin/iw").exists() {
            if let Ok(out) = Command::new("/system/bin/iw")
                .args(["dev", "wlan0", "link"])
                .output()
            {
                let text = String::from_utf8_lossy(&out.stdout);
                for raw in text.lines() {
                    let l = raw.trim();
                    if let Some(rest) = l.strip_prefix("Connected to ") {
                        info.connected = true;
                        info.bssid = rest
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_ascii_uppercase();
                    } else if let Some(s) = l.strip_prefix("SSID: ") {
                        info.ssid = s.trim().to_string();
                    } else if let Some(f) = l.strip_prefix("freq: ") {
                        info.freq_mhz = f.trim().parse().unwrap_or(0);
                    } else if let Some(sig) = l.strip_prefix("signal: ") {
                        info.rssi_dbm = sig
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                    } else if let Some(tx) = l.strip_prefix("tx bitrate: ") {
                        info.tx_mbps = tx
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0.0);
                    } else if let Some(rx) = l.strip_prefix("RX: ") {
                        info.rx_bytes = rx
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                    } else if let Some(txb) = l.strip_prefix("TX: ") {
                        info.tx_bytes = txb
                            .split_whitespace()
                            .next()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                    }
                }
            }
        }

        if let Ok(s) = fs::read_to_string("/sys/class/net/wlan0/statistics/rx_bytes") {
            if let Ok(v) = s.trim().parse::<u64>() {
                info.rx_bytes = v;
            }
        }
        if let Ok(s) = fs::read_to_string("/sys/class/net/wlan0/statistics/tx_bytes") {
            if let Ok(v) = s.trim().parse::<u64>() {
                info.tx_bytes = v;
            }
        }

        if let Some(ip) = query_iface_ipv4("wlan0") {
            info.ip = ip;
        }
        Ok(info)
    }

    fn find_process(&self, query: &str) -> Result<Option<SystemProcInfo>> {
        let Ok(entries) = fs::read_dir("/proc") else {
            return Ok(None);
        };
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let Some(pid_s) = fname.to_str() else {
                continue;
            };
            let Ok(pid) = pid_s.parse::<u32>() else {
                continue;
            };
            let cmd = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
            let cmd_str = String::from_utf8_lossy(&cmd).replace('\0', " ");
            let status_text = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();

            let mut proc_name = String::new();
            let mut rss_kb = 0u64;
            let mut threads = 1u32;
            for line in status_text.lines() {
                if let Some(n) = line.strip_prefix("Name:") {
                    proc_name = n.trim().to_string();
                } else if let Some(r) = line.strip_prefix("VmRSS:") {
                    rss_kb = r
                        .split_whitespace()
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                } else if let Some(t) = line.strip_prefix("Threads:") {
                    threads = t.trim().parse().unwrap_or(1);
                }
            }

            if cmd_str.contains(query) || proc_name.contains(query) {
                let label = cmd_str
                    .split_whitespace()
                    .next()
                    .unwrap_or(&proc_name)
                    .to_string();
                return Ok(Some(SystemProcInfo {
                    pid,
                    name: label,
                    rss_kb,
                    threads,
                }));
            }
        }
        Ok(None)
    }

    fn getprop(&self, key: &str) -> Result<Option<String>> {
        #[cfg(target_os = "android")]
        {
            use std::ffi::CString;
            extern "C" {
                fn __system_property_get(
                    name: *const libc::c_char,
                    value: *mut libc::c_char,
                ) -> libc::c_int;
            }
            let c_key = CString::new(key).context("некорректное имя свойства")?;
            let mut buf = [0u8; 96]; // PROP_VALUE_MAX = 92 в Android Bionic
            let len = unsafe {
                __system_property_get(c_key.as_ptr(), buf.as_mut_ptr().cast::<libc::c_char>())
            };
            if len > 0 {
                let s = String::from_utf8_lossy(&buf[..len as usize]).to_string();
                return Ok(Some(s));
            }
            return Ok(None);
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = key;
            Ok(None)
        }
    }

    fn setprop(&self, key: &str, value: &str) -> Result<()> {
        #[cfg(target_os = "android")]
        {
            use std::ffi::CString;
            extern "C" {
                fn __system_property_set(
                    key: *const libc::c_char,
                    value: *const libc::c_char,
                ) -> libc::c_int;
            }
            let c_key = CString::new(key).context("некорректный ключ setprop")?;
            let c_val = CString::new(value).context("некорректное значение setprop")?;
            let rc = unsafe { __system_property_set(c_key.as_ptr(), c_val.as_ptr()) };
            if rc != 0 {
                bail!("ошибка __system_property_set({key}, {value}): код {rc}");
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (key, value);
        }
        Ok(())
    }
}

/// Читает IPv4-адрес сетевого интерфейса напрямую через ядерный `ioctl(SIOCGIFADDR)` без запуска `/system/bin/ip`.
#[cfg(unix)]
fn query_iface_ipv4(iface: &str) -> Option<String> {
    #[repr(C)]
    struct IfreqAddr {
        ifr_name: [u8; 16],
        ifr_addr: libc::sockaddr_in,
        _pad: [u8; 8],
    }

    const SIOCGIFADDR: libc::c_ulong = 0x8915;
    let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if sock < 0 {
        return None;
    }

    let mut ifr: IfreqAddr = unsafe { std::mem::zeroed() };
    let bytes = iface.as_bytes();
    let copy_len = bytes.len().min(15);
    ifr.ifr_name[..copy_len].copy_from_slice(&bytes[..copy_len]);

    let rc = unsafe { libc::ioctl(sock, SIOCGIFADDR as _, &mut ifr) };
    unsafe {
        libc::close(sock);
    }
    if rc < 0 {
        return None;
    }
    let ip_be = u32::from_be(ifr.ifr_addr.sin_addr.s_addr);
    if ip_be == 0 {
        return None;
    }
    Some(format!(
        "{}.{}.{}.{}",
        (ip_be >> 24) & 0xff,
        (ip_be >> 16) & 0xff,
        (ip_be >> 8) & 0xff,
        ip_be & 0xff
    ))
}

#[cfg(not(unix))]
fn query_iface_ipv4(_iface: &str) -> Option<String> {
    None
}
