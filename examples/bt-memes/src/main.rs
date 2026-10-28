//! Эталонный Event-Driven пакет `bt-memes` на Rust + `max-sdk` (`libmax_hal`).
//!
//! Демонстрирует правильное проектирование плагинов на базе нового Event-Driven API:
//! - транзакционная подмена системных звуков в 1 строчку (`audio.override_sound(...)`)
//!   с автоматическим сохранением заводского оригинала в `MAX_PKG_DATA` и авто-откатом в `Drop`;
//! - реактивное ожидание событий ядра через `EventBus` (`rfkill`, `inotify IN_OPEN`, `evdev`, `SIGTERM`)
//!   с `0.0%` нагрузкой на CPU в режиме ожидания.

use anyhow::Result;
use max_sdk::prelude::*;
use std::path::Path;

fn main() -> Result<()> {
    let dev = max_sdk::init();
    println!(
        "[bt-memes] старт Event-Driven пакета на {} (модель={})",
        dev.name, dev.model
    );

    let mut audio = StationMaxAudioDriver::open();

    // Транзакционная подмена системных звуков (HAL сам сохраняет оригиналы и откатит их в Drop):
    audio.override_sound(
        "common/bluetooth_turning_on_jingle.opus",
        Path::new("sounds/ruko_blud.opus"),
    )?;
    audio.override_sound(
        "common/bluetooth_turning_off_jingle.opus",
        Path::new("sounds/penis_her.opus"),
    )?;
    audio.override_sound(
        "ru-RU/bluetooth_turning_on.mp3",
        Path::new("sounds/silent.mp3"),
    )?;
    audio.override_sound(
        "ru-RU/bluetooth_turning_off.mp3",
        Path::new("sounds/silent.mp3"),
    )?;
    audio.override_sound(
        "ru-RU/bluetooth_turning_blocked_by_subscription.mp3",
        Path::new("sounds/silent.mp3"),
    )?;

    let mut bus = EventBus::open(&dev)?;
    println!("[bt-memes] все 5 звуков Bluetooth переопределены, слушаем шину событий EventBus...");

    while let Some(ev) = bus.next_event(-1)? {
        match ev.kind_enum() {
            EventKind::Stop => {
                println!("[bt-memes] получен EventKind::Stop (SIGTERM), выполняем RAII-откат...");
                break;
            }
            EventKind::BtPower => {
                println!(
                    "[bt-memes] событие ядра /dev/rfkill: Bluetooth -> {}",
                    ev.name_str()
                );
            }
            EventKind::SoundPlay => {
                audio.ensure_sound_overrides();
                println!(
                    "[bt-memes] событие ядра inotify IN_OPEN: играет звук {}/{}",
                    ev.source_str(),
                    ev.name_str()
                );
            }
            EventKind::AliceState => {
                println!(
                    "[bt-memes] смена состояния Алисы: {} (code={})",
                    ev.name_str(),
                    ev.code
                );
            }
            EventKind::Input => {
                if ev.name_str() == "quadro_click" || ev.name_str() == "long_press" {
                    println!(
                        "[bt-memes] жест на кнопке {}: {} (held={} мс)",
                        ev.source_str(),
                        ev.name_str(),
                        ev.held_ms
                    );
                }
            }
            _ => {}
        }
    }

    // При выходе из main срабатывает Drop для `audio`, который автоматически
    // возвращает все 5 заводских звуков Яндекса из бэкапа и восстанавливает SY6045S.
    Ok(())
}
