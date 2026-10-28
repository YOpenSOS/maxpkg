//! Модуль `supervisor`: запуск нативных бинарных пакетов (`Rust`, `C`, `C++`, `Go`, `Zig`)
//! в foreground (`run`) и фоне (`start`), остановка (`stop` через `SIGTERM`), список (`ps`) и логи (`logs`).

use crate::manifest::PackageManifest;
use crate::store::{PackagePaths, PackageStore};
use anyhow::{bail, Context, Result};
use max_hal::Priority;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningTaskInfo {
    pub id: String,
    pub pid: u32,
    pub priority: String,
    pub lang: String,
    pub started_unix: u64,
}

impl PackageStore {
    pub fn pids_dir(&self) -> PathBuf {
        self.base_dir.join("pids")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    pub fn log_file_for(&self, id: &str) -> PathBuf {
        self.logs_dir().join(format!("{id}.log"))
    }

    fn pid_file_for(&self, id: &str) -> PathBuf {
        self.pids_dir().join(format!("{id}.json"))
    }

    fn build_native_command(
        &self,
        manifest: &PackageManifest,
        paths: &PackagePaths,
        priority: Option<&str>,
        replace: bool,
    ) -> Result<(Command, String)> {
        let bin_path = paths.root.join(&manifest.entry);
        if !bin_path.exists() {
            bail!(
                "бинарный файл пакета '{}' не найден: {}",
                manifest.id,
                bin_path.display()
            );
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755));
        }

        let eff_prio = priority
            .or(manifest.priority.as_deref())
            .map(Priority::from_str_name)
            .unwrap_or(Priority::Normal);

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("/data/local/tmp/max-platform"));
        let lib_dir = exe_dir.join("lib");
        let ld_path = format!(
            "{}:{}:/data/local/tmp/max-platform/lib:/system/lib64",
            lib_dir.display(),
            exe_dir.display()
        );

        let mut cmd = Command::new(&bin_path);
        cmd.current_dir(&paths.root);
        cmd.env("LD_LIBRARY_PATH", ld_path);
        cmd.env("MAX_PKG_ID", &manifest.id);
        cmd.env("MAX_PKG_PRIORITY", eff_prio.as_str());
        cmd.env("MAX_PKG_REPLACE", if replace { "1" } else { "0" });
        cmd.env("MAX_PKG_ROOT", &paths.root);
        cmd.env("MAX_PKG_CONFIG", &paths.config);
        cmd.env("MAX_PKG_DATA", &paths.data);
        cmd.env("MAX_PKG_TMP", &paths.tmp);

        Ok((cmd, eff_prio.as_str().to_string()))
    }

    /// Запускает нативный бинарник пакета синхронно в текущем терминале (`maxctl run` / `maxctl dev`).
    pub fn run_foreground(
        &self,
        manifest: &PackageManifest,
        paths: &PackagePaths,
        priority: Option<&str>,
        replace: bool,
    ) -> Result<()> {
        let (mut cmd, _) = self.build_native_command(manifest, paths, priority, replace)?;
        let status = cmd
            .status()
            .with_context(|| format!("не удалось запустить бинарник пакета '{}'", manifest.id))?;
        if !status.success() {
            bail!(
                "пакет '{}' завершился с кодом {:?}",
                manifest.id,
                status.code()
            );
        }
        Ok(())
    }

    /// Запускает установленный нативный пакет напрямую в фоне (`setsid`),
    /// перенаправляя `stdout` и `stderr` в `store/logs/<id>.log`.
    pub fn start_background(
        &self,
        id: &str,
        priority: Option<&str>,
        replace: bool,
    ) -> Result<RunningTaskInfo> {
        let manifest = self.load_installed(id)?;
        let paths = self.paths_for(&manifest.id, None)?;
        fs::create_dir_all(self.pids_dir())?;
        fs::create_dir_all(self.logs_dir())?;

        if let Some(existing) = self.read_running_task(id) {
            if replace {
                let _ = self.stop_background(id);
            } else {
                bail!(
                    "пакет '{}' уже запущен в фоне (PID {}, приоритет '{}'). Используйте `maxctl stop {}` или `--replace`",
                    id,
                    existing.pid,
                    existing.priority,
                    id
                );
            }
        }

        let log_path = self.log_file_for(id);
        let log_out = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&log_path)
            .with_context(|| format!("не удалось открыть лог {}", log_path.display()))?;
        let log_err = log_out.try_clone()?;

        let (mut cmd, eff_prio) =
            self.build_native_command(&manifest, &paths, priority, replace)?;
        cmd.stdin(Stdio::null()).stdout(log_out).stderr(log_err);

        #[cfg(unix)]
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        let child = cmd.spawn().context("не удалось породить фоновый процесс")?;
        let pid = child.id();

        thread::sleep(Duration::from_millis(120));
        if !is_pid_alive(pid) {
            let tail = self.read_logs(id, 20).unwrap_or_default();
            bail!(
                "фоновый пакет '{}' завершился сразу после старта:\n{}",
                id,
                tail.trim()
            );
        }

        let started_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let info = RunningTaskInfo {
            id: id.to_string(),
            pid,
            priority: eff_prio,
            lang: manifest.lang.unwrap_or_else(|| "native".to_string()),
            started_unix,
        };
        let raw = serde_json::to_string_pretty(&info)?;
        fs::write(self.pid_file_for(id), raw)?;
        Ok(info)
    }

    /// Останавливает фоновый пакет по его `id` (`SIGTERM` -> ожидание `Drop` -> `SIGKILL` при зависании).
    pub fn stop_background(&self, id: &str) -> Result<u32> {
        let pid_path = self.pid_file_for(id);
        let Some(info) = self.read_running_task(id) else {
            let _ = fs::remove_file(&pid_path);
            bail!("пакет '{}' сейчас не запущен в фоне", id);
        };

        terminate_pid_gracefully(info.pid);
        let _ = fs::remove_file(&pid_path);
        Ok(info.pid)
    }

    pub fn list_running(&self) -> Result<Vec<RunningTaskInfo>> {
        let mut out = Vec::new();
        let dir = self.pids_dir();
        if !dir.exists() {
            return Ok(out);
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(info) = serde_json::from_str::<RunningTaskInfo>(&raw) {
                    if is_pid_alive(info.pid) {
                        out.push(info);
                    } else {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    pub fn read_logs(&self, id: &str, max_lines: usize) -> Result<String> {
        let path = self.log_file_for(id);
        if !path.exists() {
            bail!("лог для пакета '{}' не найден ({})", id, path.display());
        }
        let text = fs::read_to_string(&path)?;
        let all: Vec<&str> = text.lines().collect();
        let start = all.len().saturating_sub(max_lines);
        Ok(all[start..].join("\n"))
    }

    fn read_running_task(&self, id: &str) -> Option<RunningTaskInfo> {
        let path = self.pid_file_for(id);
        let raw = fs::read_to_string(&path).ok()?;
        let info: RunningTaskInfo = serde_json::from_str(&raw).ok()?;
        if is_pid_alive(info.pid) {
            Some(info)
        } else {
            let _ = fs::remove_file(&path);
            None
        }
    }
}

fn is_pid_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    #[cfg(unix)]
    unsafe {
        if libc::kill(pid as i32, 0) != 0 {
            return false;
        }
        if let Ok(st) = fs::read_to_string(format!("/proc/{pid}/status")) {
            for line in st.lines() {
                if let Some(rest) = line.strip_prefix("State:") {
                    if rest.trim().starts_with('Z') {
                        return false;
                    }
                }
            }
        }
        true
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn terminate_pid_gracefully(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
        for _ in 0..40 {
            if !is_pid_alive(pid) {
                return;
            }
            thread::sleep(Duration::from_millis(25));
        }
        libc::kill(pid as i32, libc::SIGKILL);
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}
