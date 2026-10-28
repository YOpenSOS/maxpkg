//! Типы данных паспорта колонки (`DeviceProfile`), физических характеристик (`HardwareSpecs`)
//! и карты автоматически обнаруженных узлов ядра Linux (`KernelNodeMap`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Физические характеристики аппаратной платформы конкретной модели колонки.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSpecs {
    /// Количество управляемых RGB-светодиодов верхнего кольца (`24` на Станции Макс).
    pub led_count: u8,
    /// Тип фронтального дисплея (`"matrix"` | `"segment"` | `"none"`).
    pub screen_kind: String,
    /// Ширина матричного экрана в пикселях (`25` на Станции Макс).
    pub screen_w: u16,
    /// Высота матричного экрана в пикселях (`16` на Станции Макс).
    pub screen_h: u16,
    /// Количество физических микрофонов на плате (`7` на Станции Макс).
    pub mic_count: u8,
    /// Наличие оптического/механического энкодера громкости (`rotary-volume`).
    pub has_encoder: bool,
    /// Наличие аппаратного HDMI-выхода.
    pub has_hdmi: bool,
}

/// Полный паспорт устройства, формируемый при инициализации HAL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    /// Идентификатор модели (`"yandexstation_2"`).
    pub model: String,
    /// Человекочитаемое имя модели (`"Yandex Station Max"`).
    pub name: String,
    /// Аппаратный артикул и ревизия платы (`"YNDX-0008 (rev=4, dt=g12a_s905x2_rev35f)"`).
    pub sku: String,
    /// Серийный номер устройства из `/proc/cmdline` (`androidboot.serialno`).
    pub serial: String,
    /// Окружение ОС (`"android"` | `"linux"`).
    pub os: String,
    /// Архитектура процессора (`"aarch64"`).
    pub arch: String,
    /// Версия ядра Linux (`/proc/sys/kernel/osrelease`).
    pub kernel: String,
    /// Версия прошивки из `/system/build.prop` (`ro.build.version.incremental`).
    pub firmware: String,
    /// Физические параметры периферии.
    pub hw: HardwareSpecs,
    /// Список доступных аппаратных возможностей платы для проверки совместимости пакетов.
    pub features: Vec<String>,
    /// Карта реальных путей к символьным устройствам ядра (`/dev/input/event*`, `/dev/graphics/fb*`).
    #[serde(skip)]
    pub kernel_nodes: KernelNodeMap,
}

/// Карта обнаруженных символьных узлов ядра Linux, сопоставленная по именам драйверов в `sysfs`.
#[derive(Debug, Clone, Default)]
pub struct KernelNodeMap {
    /// Отображение имени `input`-драйвера (например, `"gpio_keypad"`, `"rotary-volume"`) в путь `/dev/input/eventX`.
    pub input_nodes: HashMap<String, String>,
    /// Отображение имени `fb`-драйвера (например, `"YNDX LED screen"`, `"YNDX LED ring"`) в путь `/dev/graphics/fbX`.
    pub fb_nodes: HashMap<String, String>,
}
