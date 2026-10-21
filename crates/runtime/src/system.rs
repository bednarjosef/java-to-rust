//! The JVM services that programs call directly: `System.getProperty`, the memory figures and
//! `gc` of `Runtime`, `Runtime.halt`, and `Thread`'s sleeps and yields. A property is answered at
//! run time from the machine it runs on, as the JVM answers it, and only for keys whose value the
//! JVM does not set itself (the translation rejects the rest). The memory figures are plausible
//! constants, not the JVM's, which depend on the machine and its flags.

use crate::exit;

/// The value of a system property this runtime answers, or `None` for any other key (which
/// `System.getProperty` gives as null).
pub fn system_property(key: &str) -> String {
    let env = |name: &str| std::env::var(name).unwrap_or_default();
    match key {
        "line.separator" => "\n".to_string(),
        "file.separator" => "/".to_string(),
        "path.separator" => ":".to_string(),
        "user.dir" => std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(),
        "user.home" => env("HOME"),
        "user.name" => env("USER"),
        "java.io.tmpdir" => "/tmp".to_string(),
        "os.name" => match std::env::consts::OS {
            "linux" => "Linux".to_string(),
            other => other.to_string(),
        },
        "os.arch" => match std::env::consts::ARCH {
            "x86_64" => "amd64".to_string(),
            "aarch64" => "aarch64".to_string(),
            other => other.to_string(),
        },
        _ => unreachable!("the translation answers only the keys it lowers: {key}"),
    }
}

/// `Runtime.totalMemory()`: the heap the runtime has, in bytes. Not the JVM's figure.
pub fn total_memory() -> i64 {
    256 << 20
}

/// `Runtime.freeMemory()`: the part of the heap that is free. Not the JVM's figure.
pub fn free_memory() -> i64 {
    192 << 20
}

/// `Runtime.maxMemory()`: the most the heap can grow to. Not the JVM's figure.
pub fn max_memory() -> i64 {
    4096 << 20
}

/// `Runtime.gc()`: there is no collector to run, so this does nothing.
pub fn gc() {}

/// `Runtime.halt(status)`: exits at once. Shutdown hooks are not run, which translated programs
/// do not have; the buffered standard output is flushed, as the JVM does.
pub fn halt(status: i32) -> ! {
    exit(status)
}

/// `Thread.sleep(millis)`. An interrupt ends it with an `InterruptedException` (see
/// `thread::try_sleep`).
pub fn sleep(millis: i64) {
    if let Err(e) = crate::thread::try_sleep(millis) {
        crate::uncaught::<()>(e);
    }
}

/// `Thread.sleep(millis, nanos)`.
pub fn sleep_nanos(millis: i64, nanos: i32) {
    if let Err(e) = crate::thread::try_sleep_nanos(millis, nanos) {
        crate::uncaught::<()>(e);
    }
}

/// `Thread.onSpinWait()`.
pub fn on_spin_wait() {
    std::hint::spin_loop();
}

/// `Thread.yield()`.
pub fn thread_yield() {
    std::thread::yield_now();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn separators_and_names_are_fixed() {
        assert_eq!(system_property("line.separator"), "\n");
        assert_eq!(system_property("file.separator"), "/");
        assert_eq!(system_property("path.separator"), ":");
        assert_eq!(system_property("java.io.tmpdir"), "/tmp");
    }

    #[test]
    fn user_dir_is_the_working_directory() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(system_property("user.dir"), cwd.to_string_lossy());
    }

    #[test]
    fn memory_figures_are_ordered() {
        assert!(free_memory() <= total_memory());
        assert!(total_memory() <= max_memory());
        assert!(free_memory() > 0);
    }

    #[test]
    fn sleep_waits_at_least_the_time_asked() {
        let start = std::time::Instant::now();
        sleep(20);
        sleep_nanos(5, 500_000);
        assert!(start.elapsed() >= Duration::from_millis(25));
    }
}
