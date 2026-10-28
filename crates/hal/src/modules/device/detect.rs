//! Автоматическая идентификация модели умной колонки и сканирование символьных устройств
//! `/sys/class/input/event*` и `/sys/class/graphics/fb*`.

use super::types::{DeviceProfile, HardwareSpecs, KernelNodeMap};
use std::fs;
use std::path::Path;

/// Определяет модель колонки, ревизию платы и пути к ядерным интерфейсам ввода/экрана.
pub fn detect_device(forced_model: Option<&str>) -> DeviceProfile {
    let arch = std::env::consts::ARCH.to_string();
    let kernel = read_kernel_version();
    let kernel_nodes = scan_kernel_nodes();

    let dt_text = [
        "/proc/device-tree/model",
        "/proc/device-tree/compatible",
        "/etc/os-release",
    ]
    .iter()
    .filter_map(|p| fs::read_to_string(p).ok())
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase();

    let is_android = Path::new("/system/build.prop").exists();
    let android_device = read_android_prop("ro.product.device");

    let raw_model = forced_model.map(String::from).unwrap_or_else(|| {
        if let Some(ref dev) = android_device {
            if !dev.is_empty() {
                return dev.clone();
            }
        }
        if Path::new("/sys/class/leds/station2:red:ring1/brightness").exists()
            || dt_text.contains("yandexstation_2")
            || kernel_nodes.fb_nodes.contains_key("YNDX LED screen")
        {
            "yandexstation_2".into()
        } else if Path::new("/sys/class/leds/cucumber:red:ring0/brightness").exists()
            || dt_text.contains("yandex,cucumber")
        {
            "yandexmidi".into()
        } else if Path::new("/sys/class/leds/mini2:red:ring1/brightness").exists()
            || dt_text.contains("yandexmini_2")
        {
            "yandexmini_2".into()
        } else if Path::new("/sys/class/leds/micro:red:ring1/brightness").exists()
            || dt_text.contains("yandexmicro")
        {
            "yandexmicro".into()
        } else {
            "yandexstation_2".into()
        }
    });

    let os = if is_android {
        "android".to_string()
    } else {
        "linux".to_string()
    };

    build_profile_for_model(&raw_model, os, arch, kernel, kernel_nodes)
}

fn build_profile_for_model(
    model: &str,
    os: String,
    arch: String,
    kernel: String,
    kernel_nodes: KernelNodeMap,
) -> DeviceProfile {
    match model {
        "yandexstation_2" => {
            let rev = read_cmdline_value("androidboot.hardware.revision")
                .unwrap_or_else(|| "unknown".into());
            let dt_id = read_cmdline_value("androidboot.hardware.amlogic_dt_id")
                .unwrap_or_else(|| "s905x2".into());
            DeviceProfile {
                model: "yandexstation_2".into(),
                name: "Yandex Station Max".into(),
                sku: format!("YNDX-0008 (rev={rev}, dt={dt_id})"),
                serial: read_cmdline_value("androidboot.serialno")
                    .unwrap_or_else(|| "UNKNOWN".into()),
                os,
                arch,
                kernel,
                firmware: read_android_prop("ro.build.version.incremental")
                    .unwrap_or_else(|| "unknown".into()),
                hw: HardwareSpecs {
                    led_count: 24,
                    screen_kind: "matrix".into(),
                    screen_w: 25,
                    screen_h: 16,
                    mic_count: 7,
                    has_encoder: true,
                    has_hdmi: true,
                },
                features: vec![
                    "device".into(),
                    "input.buttons".into(),
                    "input.encoder".into(),
                    "remote.uinput".into(),
                    "mic.loopback".into(),
                    "canvas.matrix".into(),
                    "ring.rgb".into(),
                    "audio.control".into(),
                    "sensors.ambient_light".into(),
                    "sensors.temperature".into(),
                    "bluetooth".into(),
                    "system".into(),
                ],
                kernel_nodes,
            }
        }
        "yandexmini_2" => DeviceProfile {
            model: "yandexmini_2".into(),
            name: "Yandex Station Mini 2".into(),
            sku: "YNDX-00020".into(),
            serial: "UNKNOWN".into(),
            os,
            arch,
            kernel,
            firmware: "unknown".into(),
            hw: HardwareSpecs {
                led_count: 4,
                screen_kind: "segment".into(),
                screen_w: 0,
                screen_h: 0,
                mic_count: 4,
                has_encoder: false,
                has_hdmi: false,
            },
            features: vec![
                "device".into(),
                "input.buttons".into(),
                "ring.rgb".into(),
                "audio.control".into(),
                "system".into(),
            ],
            kernel_nodes,
        },
        other => DeviceProfile {
            model: other.into(),
            name: format!("Yandex Speaker ({other})"),
            sku: "UNKNOWN".into(),
            serial: "UNKNOWN".into(),
            os,
            arch,
            kernel,
            firmware: "unknown".into(),
            hw: HardwareSpecs {
                led_count: 0,
                screen_kind: "none".into(),
                screen_w: 0,
                screen_h: 0,
                mic_count: 0,
                has_encoder: false,
                has_hdmi: false,
            },
            features: vec!["device".into(), "system".into()],
            kernel_nodes,
        },
    }
}

fn read_kernel_version() -> String {
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn read_cmdline_value(key: &str) -> Option<String> {
    let cmdline = fs::read_to_string("/proc/cmdline").ok()?;
    let prefix = format!("{key}=");
    cmdline
        .split_whitespace()
        .find_map(|part| part.strip_prefix(&prefix).map(String::from))
}

fn read_android_prop(prop: &str) -> Option<String> {
    let text = fs::read_to_string("/system/build.prop").ok()?;
    let prefix = format!("{prop}=");
    text.lines()
        .find_map(|line| line.trim().strip_prefix(&prefix).map(String::from))
}

fn scan_kernel_nodes() -> KernelNodeMap {
    let mut map = KernelNodeMap::default();
    if let Ok(entries) = fs::read_dir("/sys/class/input") {
        for entry in entries.flatten() {
            let folder = entry.file_name().to_string_lossy().to_string();
            if folder.starts_with("event") {
                if let Ok(name) = fs::read_to_string(entry.path().join("device/name")) {
                    map.input_nodes
                        .insert(name.trim().to_string(), format!("/dev/input/{folder}"));
                }
            }
        }
    }
    if let Ok(entries) = fs::read_dir("/sys/class/graphics") {
        for entry in entries.flatten() {
            let folder = entry.file_name().to_string_lossy().to_string();
            if folder.starts_with("fb") {
                if let Ok(name) = fs::read_to_string(entry.path().join("name")) {
                    map.fb_nodes
                        .insert(name.trim().to_string(), format!("/dev/graphics/{folder}"));
                }
            }
        }
    }
    map
}
