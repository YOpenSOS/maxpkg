//! Ядерные блокировки `flock` по уровням приоритета (`<resource>.<priority>.lock`),
//! проверка активного фокуса и автоматическая передача экрана/кольца между пакетами.

use super::shm::{
    clear_slot_holder, lock_file_path, locks_dir, open_or_create_shm, read_slot_holder,
    write_slot_holder,
};
use super::{ArbiterShm, Priority, ResourceKind};
use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
static SIGNALS_INSTALLED: AtomicBool = AtomicBool::new(false);
static ARBITER_CTX: OnceLock<std::sync::Mutex<(String, Priority, bool)>> = OnceLock::new();

#[cfg(unix)]
extern "C" fn on_term_signal(_sig: libc::c_int) {
    STOP_REQUESTED.store(true, Ordering::SeqCst);
}

pub fn install_signal_handlers() {
    STOP_REQUESTED.store(false, Ordering::SeqCst);
    SIGNALS_INSTALLED.store(true, Ordering::SeqCst);
    #[cfg(unix)]
    unsafe {
        libc::signal(
            libc::SIGINT,
            on_term_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            on_term_signal as *const () as libc::sighandler_t,
        );
    }
}

pub fn is_stop_requested() -> bool {
    STOP_REQUESTED.load(Ordering::Relaxed)
}

fn default_env_context() -> (String, Priority, bool) {
    if !SIGNALS_INSTALLED.load(Ordering::Relaxed) {
        install_signal_handlers();
    }
    let pkg_id = std::env::var("MAX_PKG_ID").unwrap_or_else(|_| "anonymous".to_string());
    let prio = std::env::var("MAX_PKG_PRIORITY")
        .map(|s| Priority::from_str_name(&s))
        .unwrap_or(Priority::Normal);
    let replace = std::env::var("MAX_PKG_REPLACE")
        .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    (pkg_id, prio, replace)
}

/// Устанавливает контекст текущего процесса пакета (`pkg_id`, `priority`, `replace`).
pub fn set_current_process_context(pkg_id: &str, priority: Priority, replace: bool) {
    let mtx = ARBITER_CTX.get_or_init(|| std::sync::Mutex::new(default_env_context()));
    if let Ok(mut guard) = mtx.lock() {
        *guard = (pkg_id.to_string(), priority, replace);
    }
}

pub fn current_process_context() -> (String, Priority, bool) {
    let mtx = ARBITER_CTX.get_or_init(|| std::sync::Mutex::new(default_env_context()));
    if let Ok(guard) = mtx.lock() {
        return guard.clone();
    }
    default_env_context()
}

pub struct ResourceLease {
    pub resource: ResourceKind,
    pub priority: Priority,
    #[allow(dead_code)]
    lock_file: File,
    #[allow(dead_code)]
    shm_file: Option<File>,
    shm_ptr: *mut ArbiterShm,
}

unsafe impl Send for ResourceLease {}
unsafe impl Sync for ResourceLease {}

impl ResourceLease {
    /// Захватывает слот `(resource, priority)` для текущего пакета.
    pub fn acquire(resource: ResourceKind) -> Result<Self> {
        let (pkg_id, priority, replace) = current_process_context();
        Self::acquire_with(resource, priority, &pkg_id, replace)
    }

    pub fn acquire_with(
        resource: ResourceKind,
        priority: Priority,
        pkg_id: &str,
        replace: bool,
    ) -> Result<Self> {
        let dir = locks_dir();
        fs::create_dir_all(&dir).ok();

        let (shm_file, shm_ptr) = open_or_create_shm(&dir);
        let lpath = lock_file_path(resource, priority);
        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&lpath)
            .with_context(|| format!("не удалось открыть lock-файл {}", lpath.display()))?;

        if !try_flock_ex(&lock_file) {
            let (holder_pkg, holder_pid) = read_slot_holder(shm_ptr, resource, priority);
            if replace && holder_pid > 1 {
                #[cfg(unix)]
                unsafe {
                    libc::kill(holder_pid as i32, libc::SIGTERM);
                }
                let mut acquired = false;
                for _ in 0..40 {
                    thread::sleep(Duration::from_millis(25));
                    if try_flock_ex(&lock_file) {
                        acquired = true;
                        break;
                    }
                }
                if !acquired {
                    bail!(
                        "не удалось вытеснить пакет '{}' (PID {}) с ресурса '{}'",
                        holder_pkg,
                        holder_pid,
                        resource.as_str()
                    );
                }
            } else {
                bail!(
                    "ресурс '{}' на приоритете '{}' уже занят пакетом '{}' (PID {}). Используйте --replace или другой приоритет (background / overlay)",
                    resource.as_str(),
                    priority.as_str(),
                    holder_pkg,
                    holder_pid
                );
            }
        }

        write_slot_holder(shm_ptr, resource, priority, pkg_id, std::process::id());

        Ok(Self {
            resource,
            priority,
            lock_file,
            shm_file,
            shm_ptr,
        })
    }

    /// Проверяет за ~1 мкс, является ли этот пакет текущим самым приоритетным владельцем ресурса.
    pub fn is_top_priority(&self) -> bool {
        let p_idx = self.priority as usize;
        for higher_idx in (p_idx + 1)..3 {
            let higher_prio = match higher_idx {
                1 => Priority::Normal,
                _ => Priority::Overlay,
            };
            if is_resource_locked(self.resource, higher_prio) {
                return false;
            }
        }
        true
    }

    /// Проверяет, есть ли хоть один живой держатель этого ресурса на любом приоритете кроме нас.
    pub fn any_other_priority_active(&self) -> Option<Priority> {
        let my_idx = self.priority as usize;
        for idx in (0..3).rev() {
            if idx == my_idx {
                continue;
            }
            let pr = match idx {
                0 => Priority::Background,
                1 => Priority::Normal,
                _ => Priority::Overlay,
            };
            if is_resource_locked(self.resource, pr) {
                return Some(pr);
            }
        }
        None
    }

    pub fn shm_mut(&self) -> Option<&mut ArbiterShm> {
        if self.shm_ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *self.shm_ptr })
        }
    }
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        clear_slot_holder(self.shm_ptr, self.resource, self.priority);
        #[cfg(unix)]
        unsafe {
            use std::os::unix::io::AsRawFd;
            libc::flock(self.lock_file.as_raw_fd(), libc::LOCK_UN);
        }
        #[cfg(unix)]
        if !self.shm_ptr.is_null() {
            unsafe {
                libc::munmap(
                    self.shm_ptr as *mut libc::c_void,
                    std::mem::size_of::<ArbiterShm>(),
                );
            }
        }
    }
}

pub fn is_resource_locked(resource: ResourceKind, priority: Priority) -> bool {
    let lpath = lock_file_path(resource, priority);
    let Ok(file) = OpenOptions::new().read(true).write(true).open(lpath) else {
        return false;
    };
    if try_flock_ex(&file) {
        #[cfg(unix)]
        unsafe {
            use std::os::unix::io::AsRawFd;
            libc::flock(file.as_raw_fd(), libc::LOCK_UN);
        }
        false
    } else {
        true
    }
}

fn try_flock_ex(file: &File) -> bool {
    #[cfg(unix)]
    unsafe {
        use std::os::unix::io::AsRawFd;
        libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        true
    }
}

/// Возвращает список всех активных ядерных блокировок `(resource, priority, pkg_id, pid)`.
pub fn inspect_active_locks() -> Vec<(ResourceKind, Priority, String, u32)> {
    let dir = locks_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let (_file, shm_ptr) = open_or_create_shm(&dir);
    let mut out = Vec::new();
    let resources = [
        ResourceKind::Canvas,
        ResourceKind::Ring,
        ResourceKind::Input,
        ResourceKind::Amp,
    ];
    let priorities = [Priority::Background, Priority::Normal, Priority::Overlay];

    for &res in &resources {
        for &prio in &priorities {
            if is_resource_locked(res, prio) {
                let (pkg, pid) = read_slot_holder(shm_ptr, res, prio);
                out.push((res, prio, pkg, pid));
            }
        }
    }

    #[cfg(unix)]
    if !shm_ptr.is_null() {
        unsafe {
            libc::munmap(
                shm_ptr as *mut libc::c_void,
                std::mem::size_of::<ArbiterShm>(),
            );
        }
    }
    out
}
