//! Консольный пакетный менеджер и супервизор нативных пакетов `maxctl`.

use anyhow::Result;
use clap::{Parser, Subcommand};
use max_hal::modules::arbiter::lease::inspect_active_locks;
use max_hal::{BoardCapabilities, HAL_MODULE_SLOTS};
use max_pkg::{PackageManifest, PackageStore};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "maxctl",
    about = "Пакетный менеджер и супервизор нативных пакетов max-platform (Rust / C / C++ / Go / Zig)"
)]
struct Cli {
    /// Принудительно задать модель колонки (например, yandexstation_2)
    #[arg(long, global = true)]
    model: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Показать паспорт колонки (`device`), обнаруженные узлы ядра и статус модулей HAL
    Info,
    /// Установить нативный пакет в хранилище колонки
    Install {
        /// Путь к папке пакета
        path: PathBuf,
    },
    /// Показать список установленных пакетов
    List,
    /// Запустить установленный пакет в текущем терминале (foreground)
    Run {
        /// Идентификатор установленного пакета
        id: String,
        /// Приоритет доступа к железу: background | normal | overlay
        #[arg(long)]
        priority: Option<String>,
        /// Вытеснить уже работающий пакет на том же уровне приоритета (SIGTERM + перехват лока)
        #[arg(long)]
        replace: bool,
    },
    /// Запустить установленный пакет в фоне (демоном) с записью в лог
    Start {
        /// Идентификатор установленного пакета
        id: String,
        /// Приоритет доступа к железу: background | normal | overlay
        #[arg(long)]
        priority: Option<String>,
        /// Вытеснить уже работающий пакет на том же уровне приоритета
        #[arg(long)]
        replace: bool,
    },
    /// Аккуратно остановить фоновый пакет (SIGTERM -> штатный Drop/RAII откат железа)
    Stop {
        /// Идентификатор пакета
        id: String,
    },
    /// Показать все работающие фоновые пакеты и активные блокировки железа (canvas, ring, input, amp)
    Ps,
    /// Показать лог фонового пакета
    Logs {
        /// Идентификатор пакета
        id: String,
        /// Количество последних строк
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
    },
    /// Удалить установленный пакет
    Remove {
        /// Идентификатор пакета
        id: String,
    },
    /// Запустить пакет напрямую из папки без установки (dev-режим)
    Dev {
        /// Путь к папке пакета с `package.toml`
        path: PathBuf,
        /// Приоритет доступа к железу: background | normal | overlay
        #[arg(long)]
        priority: Option<String>,
        /// Вытеснить уже работающий пакет на том же уровне приоритета
        #[arg(long)]
        replace: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let board = BoardCapabilities::detect(cli.model.as_deref());
    let store = PackageStore::for_device(&board.profile, None);

    match cli.command {
        Commands::Info => {
            println!("=== Паспорт колонки (device) ===");
            println!("Модель:      {} ({})", board.profile.name, board.profile.model);
            println!("SKU / Сер.:  {} / {}", board.profile.sku, board.profile.serial);
            println!(
                "Система:     os={}, arch={}, kernel={}, fw={}",
                board.profile.os, board.profile.arch, board.profile.kernel, board.profile.firmware
            );
            println!(
                "Железо (hw): диодов={}, экран={} ({}x{}), микрофонов={}, энкодер={}, hdmi={}",
                board.profile.hw.led_count,
                board.profile.hw.screen_kind,
                board.profile.hw.screen_w,
                board.profile.hw.screen_h,
                board.profile.hw.mic_count,
                board.profile.hw.has_encoder,
                board.profile.hw.has_hdmi,
            );
            if !board.profile.kernel_nodes.input_nodes.is_empty()
                || !board.profile.kernel_nodes.fb_nodes.is_empty()
            {
                println!("\n=== Автоматически найденные узлы ядра Linux ===");
                for (name, node) in &board.profile.kernel_nodes.input_nodes {
                    println!("  [input] {:<28} -> {}", name, node);
                }
                for (name, node) in &board.profile.kernel_nodes.fb_nodes {
                    println!("  [fb]    {:<28} -> {}", name, node);
                }
            }
            println!("\n=== Зарегистрированные модули libmax_hal (C / C++ / Go / Zig / Rust) ===");
            for &(name, req_cap) in HAL_MODULE_SLOTS {
                let avail = req_cap.map(|c| board.has_capability(c)).unwrap_or(true);
                println!(
                    "  - {:<12} : {}",
                    name,
                    if avail { "ДОСТУПЕН" } else { "нет на плате" }
                );
            }
        }
        Commands::Install { path } => {
            let manifest = store.install(&path, &board.profile)?;
            println!(
                "[OK] Пакет '{}' v{} ({}) успешно установлен в {}",
                manifest.id,
                manifest.version,
                manifest.name,
                store.pkgs_dir().join(&manifest.id).display()
            );
        }
        Commands::List => {
            let pkgs = store.list()?;
            if pkgs.is_empty() {
                println!("Нет установленных пакетов.");
            } else {
                println!("Установленные пакеты ({}):", pkgs.len());
                for m in pkgs {
                    let lang = m.lang.as_deref().unwrap_or("native");
                    println!(
                        "  - {:<16} v{:<8} [{}] | {}",
                        m.id, m.version, lang, m.name
                    );
                }
            }
        }
        Commands::Run {
            id,
            priority,
            replace,
        } => {
            let manifest = store.load_installed(&id)?;
            let paths = store.paths_for(&manifest.id, None)?;
            store.run_foreground(&manifest, &paths, priority.as_deref(), replace)?;
        }
        Commands::Start {
            id,
            priority,
            replace,
        } => {
            let info = store.start_background(&id, priority.as_deref(), replace)?;
            println!(
                "[OK] Пакет '{}' [{}] запущен в фоне (PID={}, приоритет='{}', лог={})",
                info.id,
                info.lang,
                info.pid,
                info.priority,
                store.log_file_for(&info.id).display()
            );
        }
        Commands::Stop { id } => {
            let pid = store.stop_background(&id)?;
            println!(
                "[OK] Пакет '{}' (PID={}) аккуратно остановлен (SIGTERM -> Drop).",
                id, pid
            );
        }
        Commands::Ps => {
            let tasks = store.list_running()?;
            println!("=== Фоновые пакеты ({}) ===", tasks.len());
            if tasks.is_empty() {
                println!("  (нет активных фоновых пакетов)");
            } else {
                for t in &tasks {
                    println!(
                        "  - {:<16} | PID={:<6} | lang={:<6} | priority={:<10}",
                        t.id, t.pid, t.lang, t.priority
                    );
                }
            }
            let locks = inspect_active_locks();
            println!("\n=== Активные аппаратные блокировки (flock + arbiter.shm) ===");
            if locks.is_empty() {
                println!("  (все аппаратные ресурсы свободны)");
            } else {
                for (res, prio, pkg, pid) in locks {
                    println!(
                        "  - ресурс={:<8} | приоритет={:<10} | владелец='{}' (PID={})",
                        res.as_str(),
                        prio.as_str(),
                        pkg,
                        pid
                    );
                }
            }
        }
        Commands::Logs { id, lines } => {
            let out = store.read_logs(&id, lines)?;
            println!("{out}");
        }
        Commands::Remove { id } => {
            let _ = store.stop_background(&id);
            store.remove(&id)?;
            println!("[OK] Пакет '{}' удалён.", id);
        }
        Commands::Dev {
            path,
            priority,
            replace,
        } => {
            let manifest = PackageManifest::load_from_dir(&path)?;
            let paths = store.paths_for(&manifest.id, Some(&path))?;
            store.run_foreground(&manifest, &paths, priority.as_deref(), replace)?;
        }
    }

    Ok(())
}
