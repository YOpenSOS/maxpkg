//! Драйвер датчиков `sysfs` / `IIO` для Станции Макс и Linux-колонок.
//! Обеспечивает автоматический возврат настроек датчика освещённости `ltr308als01` в `Drop`.

use super::{LightReading, SensorItem, SensorsHal, ThermalReading};
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

const IIO_LIGHT_DIR: &str = "/sys/bus/iio/devices/iio:device0";
const CPU_FREQ_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";
const RTC_EPOCH_PATH: &str = "/sys/class/rtc/rtc0/since_epoch";

/// Драйвер чтения и калибровки бортовых датчиков через ядерные интерфейсы `IIO` и `sysfs`.
pub struct SysfsSensorsDriver {
    light_dir: Option<PathBuf>,
    saved_gain: Option<String>,
    saved_int_time: Option<String>,
    modified_light: bool,
}

impl SysfsSensorsDriver {
    pub fn open() -> Self {
        let light_dir = find_iio_light_dir();
        let saved_gain = light_dir
            .as_ref()
            .and_then(|d| fs::read_to_string(d.join("in_intensity_hardwaregain")).ok())
            .map(|s| s.trim().to_string());
        let saved_int_time = light_dir
            .as_ref()
            .and_then(|d| fs::read_to_string(d.join("in_intensity_integration_time")).ok())
            .map(|s| s.trim().to_string());

        Self {
            light_dir,
            saved_gain,
            saved_int_time,
            modified_light: false,
        }
    }
}

fn find_iio_light_dir() -> Option<PathBuf> {
    let default = Path::new(IIO_LIGHT_DIR);
    if default.join("in_intensity_input").exists() {
        return Some(default.to_path_buf());
    }
    let entries = fs::read_dir("/sys/bus/iio/devices").ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.join("in_intensity_input").exists() {
            return Some(p);
        }
    }
    None
}

fn read_f32_file(path: &Path) -> Option<f32> {
    fs::read_to_string(path).ok()?.trim().parse::<f32>().ok()
}

fn read_u32_file(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse::<u32>().ok()
}

impl SensorsHal for SysfsSensorsDriver {
    fn read_light(&self) -> Result<LightReading> {
        let Some(ref dir) = self.light_dir else {
            bail!("датчик освещённости IIO (/sys/bus/iio/devices/iio:device0) не найден");
        };
        let lux = read_f32_file(&dir.join("in_intensity_input")).unwrap_or(0.0);
        let raw = read_u32_file(&dir.join("in_intensity_raw")).unwrap_or(0);
        let gain = read_u32_file(&dir.join("in_intensity_hardwaregain")).unwrap_or(3) as u8;
        let int_sec = read_f32_file(&dir.join("in_intensity_integration_time")).unwrap_or(0.1);
        let integration_ms = (int_sec * 1000.0).round() as u16;
        Ok(LightReading {
            lux,
            raw,
            gain,
            integration_ms,
        })
    }

    fn configure_light(&mut self, gain: u8, integration_ms: u16) -> Result<()> {
        if ![1, 3, 6, 9, 18].contains(&gain) {
            bail!("gain датчика света должен быть 1, 3, 6, 9 или 18");
        }
        if ![25, 50, 100, 200, 400].contains(&integration_ms) {
            bail!("integration_ms должен быть 25, 50, 100, 200 или 400 мс");
        }
        let Some(ref dir) = self.light_dir else {
            bail!("датчик освещённости IIO не найден");
        };
        fs::write(dir.join("in_intensity_hardwaregain"), gain.to_string())?;
        fs::write(
            dir.join("in_intensity_integration_time"),
            format!("0.{integration_ms:03}"),
        )?;
        self.modified_light = true;
        Ok(())
    }

    fn read_thermal(&self) -> Result<ThermalReading> {
        let mut soc_c = 0.0;
        let mut ddr_c = 0.0;
        let mut wifi_c = 0.0;

        let entries = fs::read_dir("/sys/class/thermal")?;
        for entry in entries.flatten() {
            let p = entry.path();
            if let (Ok(kind), Some(milli_c)) = (
                fs::read_to_string(p.join("type")),
                read_f32_file(&p.join("temp")),
            ) {
                let c = (milli_c / 100.0).round() / 10.0;
                match kind.trim() {
                    "soc_thermal" => soc_c = c,
                    "ddr_thermal" => ddr_c = c,
                    "iwlwifi_1" | "wifi" => wifi_c = c,
                    _ => {}
                }
            }
        }

        Ok(ThermalReading {
            soc_c,
            ddr_c,
            wifi_c,
        })
    }

    fn read_cpu_freq_mhz(&self) -> Result<u32> {
        let Some(khz) = read_u32_file(Path::new(CPU_FREQ_PATH)) else {
            bail!("не удалось прочитать {CPU_FREQ_PATH}");
        };
        Ok(khz / 1000)
    }

    fn read_rtc_epoch(&self) -> Result<u64> {
        let raw = fs::read_to_string(RTC_EPOCH_PATH)?;
        let epoch = raw.trim().parse::<u64>()?;
        Ok(epoch)
    }

    fn list_all(&self) -> Result<Vec<SensorItem>> {
        let light = self.read_light()?;
        let therm = self.read_thermal()?;
        let cpu_mhz = self.read_cpu_freq_mhz()?;
        Ok(vec![
            SensorItem {
                id: "ambient_light".into(),
                kind: "light".into(),
                unit: "lux".into(),
                value: light.lux,
            },
            SensorItem {
                id: "soc_thermal".into(),
                kind: "temperature".into(),
                unit: "celsius".into(),
                value: therm.soc_c,
            },
            SensorItem {
                id: "ddr_thermal".into(),
                kind: "temperature".into(),
                unit: "celsius".into(),
                value: therm.ddr_c,
            },
            SensorItem {
                id: "wifi_thermal".into(),
                kind: "temperature".into(),
                unit: "celsius".into(),
                value: therm.wifi_c,
            },
            SensorItem {
                id: "cpu_freq".into(),
                kind: "frequency".into(),
                unit: "MHz".into(),
                value: cpu_mhz as f32,
            },
        ])
    }
}

/// Возвращает заводские параметры усиления и времени накопления `ltr308als01` при закрытии дескриптора.
impl Drop for SysfsSensorsDriver {
    fn drop(&mut self) {
        if self.modified_light {
            if let Some(ref dir) = self.light_dir {
                if let Some(ref g) = self.saved_gain {
                    let _ = fs::write(dir.join("in_intensity_hardwaregain"), g);
                }
                if let Some(ref it) = self.saved_int_time {
                    let _ = fs::write(dir.join("in_intensity_integration_time"), it);
                }
            }
        }
    }
}
