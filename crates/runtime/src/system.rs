//! The JVM services that programs call directly: `System.getProperty`, `System.getenv`, the
//! memory figures and
//! `gc` of `Runtime`, `Runtime.halt`, and `Thread`'s sleeps and yields. A property whose value
//! depends on the machine or the run is answered here from the machine it runs on, as the JVM
//! answers it; the translation fixes the others (see `lower::system`). The memory figures are plausible
//! constants, not the JVM's, which depend on the machine and its flags.

/// `System.getenv(name)`: the variable of the environment the program runs in, as the JVM reads
/// it, or `None` where it is unset. A name that no variable can have (empty, or with `=` or a
/// zero byte) is unset, and a value that is not UTF-8 is read with replacement characters.
pub fn getenv(name: &crate::Wtf8) -> Option<crate::JString> {
    let name = &*name.to_string_lossy();
    if name.is_empty() || name.contains(['=', '\0']) {
        return None;
    }
    std::env::var_os(name).map(|value| value.to_string_lossy().into_owned().into())
}

/// The entries of the environment the program runs in, as the JVM reads them for
/// `System.getenv()`: each `name=value` as the C library's `environ` has it, followed by a zero
/// byte, in its order. The patch of `ProcessEnvironment` splits them (`environ0`).
pub fn environ() -> Vec<i8> {
    unsafe extern "C" {
        static environ: *const *const std::ffi::c_char;
    }
    let mut bytes = Vec::new();
    // SAFETY: `environ` is a null-terminated array of C strings; a translated program never
    // changes its environment, so nothing writes the array while it is read.
    unsafe {
        let mut entry = environ;
        while !entry.is_null() && !(*entry).is_null() {
            let text = std::ffi::CStr::from_ptr(*entry).to_bytes();
            bytes.extend(text.iter().map(|&b| b as i8));
            bytes.push(0);
            entry = entry.add(1);
        }
    }
    bytes
}

/// The value of a system property that depends on the machine or the run (the translation fixes
/// the others, see `lower::system`).
pub fn system_property(key: &str) -> crate::JString {
    property(key).into()
}

fn property(key: &str) -> String {
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
        // The kernel's release, which the JVM reads with `uname`.
        "os.version" => std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|v| v.trim().to_string())
            .unwrap_or_default(),
        "os.arch" => match std::env::consts::ARCH {
            "x86_64" => "amd64".to_string(),
            "aarch64" => "aarch64".to_string(),
            other => other.to_string(),
        },
        _ => unreachable!("the translation answers only the keys it lowers: {key}"),
    }
}

/// `System.getProperty("java.home")`: the `JAVA_HOME` of the run, or else `fallback`, the home of
/// the JDK the translator ran with.
pub fn java_home(fallback: &crate::Wtf8) -> crate::JString {
    match std::env::var("JAVA_HOME").ok().filter(|home| !home.is_empty()) {
        Some(home) => home.into(),
        None => fallback.to_owned(),
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

/// `Runtime.gc()`: does nothing. The collector (`gc.rs`) runs on its own when the program has
/// allocated enough, which the specification of `gc()` allows.
pub fn gc() {}

/// `Runtime.halt(status)`: exits at once, without running the shutdown hooks, which only
/// `System.exit` and the other ways out run (`thread::run_shutdown_hooks`). What standard output
/// has written is written out, as the JVM's unbuffered stream has written it.
pub fn halt(status: i32) -> ! {
    crate::drain_stdout();
    std::process::exit(status)
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
        assert_eq!(system_property("user.dir"), *cwd.to_string_lossy());
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
