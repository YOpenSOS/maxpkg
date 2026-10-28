//! Драйвер `BluedroidDriver`: читает `/data/misc/bluedroid/bt_config.conf` и `/sys/class/bluetooth/hci0`
//! напрямую из файловой системы без запуска `dumpsys`, и управляет питанием через `svc bluetooth`
//! с автоматическим возвратом исходного состояния в `Drop`.

use super::{BluetoothDevice, BluetoothHal, BluetoothStatus};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

const BT_CONFIG_PATH: &str = "/data/misc/bluedroid/bt_config.conf";
const HCI0_SYSFS_DIR: &str = "/sys/class/bluetooth/hci0";

/// Аппаратный драйвер Bluetooth-стека Android Bluedroid и интерфейса ядра `hci0`.
pub struct BluedroidDriver {
    saved_power: Option<bool>,
}

impl BluedroidDriver {
    pub fn open() -> Self {
        Self { saved_power: None }
    }

    fn read_hci0_state(&self) -> (bool, bool) {
        let hci0 = Path::new(HCI0_SYSFS_DIR);
        if !hci0.exists() {
            return (false, false);
        }

        let mut enabled = true;
        let mut connected = false;

        if let Ok(entries) = fs::read_dir(hci0) {
            for entry in entries.flatten() {
                let fname = entry.file_name();
                let Some(s) = fname.to_str() else {
                    continue;
                };
                if s.starts_with("rfkill") {
                    if let Ok(st) = fs::read_to_string(entry.path().join("state")) {
                        enabled = st.trim() == "1";
                    }
                } else if s.starts_with("hci0:") {
                    connected = true;
                }
            }
        }
        (enabled, connected)
    }

    fn parse_bt_config(&self) -> Result<(String, String, u8, Vec<BluetoothDevice>)> {
        let text = fs::read_to_string(BT_CONFIG_PATH)
            .with_context(|| format!("не удалось прочитать {BT_CONFIG_PATH}"))?;

        let mut adapter_addr = String::new();
        let mut adapter_name = String::new();
        let mut scan_mode = 2u8;
        let mut devices = Vec::new();

        let mut cur_sec = String::new();
        let mut cur_name = String::new();
        let mut cur_dev_type = 1u8;
        let mut cur_ts = 0u64;

        let flush_device = |sec: &str,
                            name: &str,
                            dev_type: u8,
                            ts: u64,
                            out: &mut Vec<BluetoothDevice>| {
            if is_mac_section(sec) {
                let kind = match dev_type {
                    2 => "LE",
                    3 => "DUAL",
                    _ => "BR/EDR",
                };
                out.push(BluetoothDevice {
                    mac: sec.to_ascii_uppercase(),
                    name: if name.is_empty() {
                        sec.to_ascii_uppercase()
                    } else {
                        name.to_string()
                    },
                    kind: kind.to_string(),
                    timestamp: ts,
                });
            }
        };

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                flush_device(&cur_sec, &cur_name, cur_dev_type, cur_ts, &mut devices);
                cur_sec = line[1..line.len() - 1].trim().to_string();
                cur_name.clear();
                cur_dev_type = 1;
                cur_ts = 0;
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let val = v.trim();

            if cur_sec.eq_ignore_ascii_case("Adapter") {
                match key {
                    "Address" => adapter_addr = val.to_ascii_uppercase(),
                    "Name" => adapter_name = val.to_string(),
                    "ScanMode" => scan_mode = val.parse().unwrap_or(2),
                    _ => {}
                }
            } else if is_mac_section(&cur_sec) {
                match key {
                    "Name" | "AliasedName" if cur_name.is_empty() => cur_name = val.to_string(),
                    "DevType" => cur_dev_type = val.parse().unwrap_or(1),
                    "Timestamp" => cur_ts = val.parse().unwrap_or(0),
                    _ => {}
                }
            }
        }
        flush_device(&cur_sec, &cur_name, cur_dev_type, cur_ts, &mut devices);

        Ok((adapter_addr, adapter_name, scan_mode, devices))
    }

    fn apply_power(&mut self, on: bool) -> Result<()> {
        if !Path::new("/system/bin/svc").exists() {
            bail!("системная утилита /system/bin/svc недоступна");
        }
        let arg = if on { "enable" } else { "disable" };
        Command::new("/system/bin/svc")
            .args(["bluetooth", arg])
            .output()
            .context("ошибка вызова /system/bin/svc bluetooth")?;
        Ok(())
    }
}

fn is_mac_section(s: &str) -> bool {
    s.len() == 17 && s.as_bytes().iter().filter(|&&b| b == b':').count() == 5
}

impl BluetoothHal for BluedroidDriver {
    fn status(&self) -> Result<BluetoothStatus> {
        let (enabled, connected) = self.read_hci0_state();
        let (address, name, scan_mode, devices) = self.parse_bt_config()?;
        Ok(BluetoothStatus {
            enabled,
            address,
            name,
            scan_mode,
            connected,
            bonded_count: devices.len(),
        })
    }

    fn devices(&self) -> Result<Vec<BluetoothDevice>> {
        let (_, _, _, devices) = self.parse_bt_config()?;
        Ok(devices)
    }

    fn set_power(&mut self, on: bool) -> Result<()> {
        if self.saved_power.is_none() {
            let (cur_on, _) = self.read_hci0_state();
            self.saved_power = Some(cur_on);
        }
        self.apply_power(on)
    }
}

impl Drop for BluedroidDriver {
    fn drop(&mut self) {
        if let Some(orig) = self.saved_power.take() {
            let _ = self.apply_power(orig);
        }
    }
}
