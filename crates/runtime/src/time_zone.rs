//! `TimeZone`'s two natives, which tell the local time zone the way the JDK does on Linux:
//! `getSystemTimeZoneID` names the zone from the environment and the system's files, and
//! `getSystemGMTOffsetID` gives the offset from UTC when that name is not a zone the JDK knows.

use std::ffi::c_char;
use std::time::{SystemTime, UNIX_EPOCH};

/// The prefix of a zone file's path that the zone's name follows.
const ZONEINFO_DIR: &str = "zoneinfo/";

/// C's `struct tm` on 64-bit Linux (glibc and musl alike).
#[repr(C)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const c_char,
}

unsafe extern "C" {
    fn tzset();
    fn localtime_r(time: *const i64, tm: *mut Tm) -> *mut Tm;
}

/// `TimeZone.getSystemTimeZoneID(javaHome)`: the `TZ` variable if it is set and not empty, without
/// a leading `:`; else the first line of `/etc/timezone`; else the name that follows `zoneinfo/`
/// in the target of the `/etc/localtime` link; else `None`. The JDK's `javaHome` argument names
/// only its `tzmappings` file, which this JDK does not have, so it is not read.
pub fn system_time_zone_id() -> Option<String> {
    if let Some(tz) = std::env::var_os("TZ").filter(|tz| !tz.is_empty()) {
        let tz = tz.to_string_lossy();
        return Some(tz.strip_prefix(':').unwrap_or(&tz).to_string());
    }
    if let Some(name) = std::fs::read("/etc/timezone").ok().and_then(|bytes| first_line(&bytes)) {
        return Some(name);
    }
    let link = std::fs::read_link("/etc/localtime").ok()?;
    zone_after_zoneinfo(&link.to_string_lossy()).map(str::to_string)
}

/// `TimeZone.getSystemGMTOffsetID()`: `GMT` for no offset, else `GMT+hh:mm` or `GMT-hh:mm`, for
/// the offset of the local time from UTC now.
pub fn system_gmt_offset_id() -> String {
    gmt_offset_id(local_offset_seconds())
}

/// The first line of a file's bytes, without its newline; `None` if it is empty.
fn first_line(bytes: &[u8]) -> Option<String> {
    let line = bytes.split(|&b| b == b'\n').next()?;
    (!line.is_empty()).then(|| String::from_utf8_lossy(line).into_owned())
}

/// The zone name in a path to a zone file: what follows the first `zoneinfo/`.
fn zone_after_zoneinfo(path: &str) -> Option<&str> {
    path.find(ZONEINFO_DIR).map(|at| &path[at + ZONEINFO_DIR.len()..])
}

/// The `GMT` form of an offset in seconds east of UTC. Only whole minutes count, truncated
/// towards zero as the JDK's. (For an offset with seconds the JDK's output depends on the clock,
/// which this does not reproduce.)
fn gmt_offset_id(seconds: i64) -> String {
    let minutes = seconds / 60;
    if minutes == 0 {
        return "GMT".to_string();
    }
    let sign = if minutes < 0 { '-' } else { '+' };
    let minutes = minutes.abs();
    format!("GMT{sign}{:02}:{:02}", minutes / 60, minutes % 60)
}

/// The offset of the local time from UTC now, in seconds, as the C library gives it after
/// `tzset()` (from `TZ`, else `/etc/localtime`). Zero if the C library cannot tell.
fn local_offset_seconds() -> i64 {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    // SAFETY: `tzset` takes no arguments; `localtime_r` writes a `Tm` it is given, and `now` lives
    // for the call. The `Tm` is only read when the call returns it.
    unsafe {
        tzset();
        let mut tm: Tm = std::mem::zeroed();
        if localtime_r(&now, &mut tm).is_null() { 0 } else { tm.tm_gmtoff }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_line_of_the_timezone_file_is_its_zone() {
        assert_eq!(first_line(b"Europe/Berlin\n"), Some("Europe/Berlin".to_string()));
        assert_eq!(first_line(b"Europe/Berlin"), Some("Europe/Berlin".to_string()));
        assert_eq!(first_line(b"\nEurope/Berlin\n"), None);
        assert_eq!(first_line(b""), None);
    }

    #[test]
    fn the_zone_follows_zoneinfo_in_the_link() {
        assert_eq!(zone_after_zoneinfo("/usr/share/zoneinfo/Europe/Prague"), Some("Europe/Prague"));
        assert_eq!(zone_after_zoneinfo("/usr/share/zoneinfo/UTC"), Some("UTC"));
        assert_eq!(zone_after_zoneinfo("/etc/localtime"), None);
    }

    #[test]
    fn offsets_are_written_as_the_jdk_writes_them() {
        assert_eq!(gmt_offset_id(0), "GMT");
        assert_eq!(gmt_offset_id(7200), "GMT+02:00");
        assert_eq!(gmt_offset_id(-18000), "GMT-05:00");
        assert_eq!(gmt_offset_id(19800), "GMT+05:30");
        assert_eq!(gmt_offset_id(-60), "GMT-00:01");
        assert_eq!(gmt_offset_id(-14 * 3600), "GMT-14:00");
        // Seconds are dropped, towards zero.
        assert_eq!(gmt_offset_id(1172), "GMT+00:19");
        assert_eq!(gmt_offset_id(-59), "GMT");
    }

    #[test]
    fn the_gmt_offset_id_has_the_jdk_shape() {
        let id = system_gmt_offset_id();
        assert!(id == "GMT" || (id.len() == 9 && (id.starts_with("GMT+") || id.starts_with("GMT-"))), "{id}");
    }
}
