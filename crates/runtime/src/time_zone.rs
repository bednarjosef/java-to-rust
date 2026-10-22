//! `TimeZone`'s two natives, which tell the local time zone the way the JDK does on Linux:
//! `getSystemTimeZoneID` names the zone from the environment and the system's files, and
//! `getSystemGMTOffsetID` gives the offset from UTC when that name is not a zone the JDK knows.

use std::ffi::c_char;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The prefix of a zone file's path that the zone's name follows.
const ZONEINFO_DIR: &str = "zoneinfo/";

/// The file that names the system zone: a link into the zone directory, or a copy of a zone file.
const LOCALTIME: &str = "/etc/localtime";

/// The directory of zone files that a copied `/etc/localtime` is searched for in.
const ZONE_DIR: &str = "/usr/share/zoneinfo";

/// Entries that the search of the zone directory skips at any depth, as the JDK does: `posixrules`
/// and `localtime` are copies or links of a zone, and `posix` and `right` repeat every zone.
const SKIPPED_ENTRIES: [&str; 4] = ["posixrules", "localtime", "posix", "right"];

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
/// a leading `:`; else the first line of `/etc/timezone`; else the zone of `/etc/localtime` (see
/// `zone_of_localtime`); else `None`. The JDK's `javaHome` argument names only its `tzmappings`
/// file, which this JDK does not have, so it is not read.
pub fn system_time_zone_id() -> Option<String> {
    if let Some(tz) = std::env::var_os("TZ").filter(|tz| !tz.is_empty()) {
        let tz = tz.to_string_lossy();
        return Some(tz.strip_prefix(':').unwrap_or(&tz).to_string());
    }
    if let Some(name) = std::fs::read("/etc/timezone").ok().and_then(|bytes| first_line(&bytes)) {
        return Some(name);
    }
    zone_of_localtime(Path::new(LOCALTIME), Path::new(ZONE_DIR))
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

/// The zone of the localtime file: what follows `zoneinfo/` in its link's target or, when it is a
/// regular file (a copy of a zone file), the path under `zone_dir` of the zone file with its size
/// and bytes. `None` if it is neither, or no zone file matches.
fn zone_of_localtime(localtime: &Path, zone_dir: &Path) -> Option<String> {
    let kind = std::fs::symlink_metadata(localtime).ok()?;
    if kind.is_symlink() {
        let link = std::fs::read_link(localtime).ok()?;
        zone_after_zoneinfo(&link.to_string_lossy()).map(str::to_string)
    } else if kind.is_file() {
        let bytes = std::fs::read(localtime).ok()?;
        find_zone_file(zone_dir, "", &bytes)
    } else {
        None
    }
}

/// The name, under `dir` and with `prefix` before it, of the first regular file under `dir` whose
/// bytes are `bytes`. Entries are taken in the order the directory lists them, as the JDK's search
/// does, so when a zone has several identical copies the first listed is named. Links to
/// directories are not followed, so a loop of links ends.
fn find_zone_file(dir: &Path, prefix: &str, bytes: &[u8]) -> Option<String> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if SKIPPED_ENTRIES.contains(&file_name.as_str()) {
            continue;
        }
        let name = format!("{prefix}{file_name}");
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            if let Some(found) = find_zone_file(&entry.path(), &format!("{name}/"), bytes) {
                return Some(found);
            }
        } else if is_copy_of(&entry.path(), bytes) {
            return Some(name);
        }
    }
    None
}

/// Whether `path`, followed through links as `stat` follows them, is a regular file of `bytes`.
fn is_copy_of(path: &Path, bytes: &[u8]) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    // The size is compared first, so that most files are not read.
    meta.is_file() && meta.len() == bytes.len() as u64 && std::fs::read(path).is_ok_and(|zone| zone == bytes)
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
    use std::path::PathBuf;

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

    /// Zone files for the copy tests: two of the same size with other bytes, and one longer.
    const PRAGUE: &[u8] = b"TZif Prague";
    const BERLIN: &[u8] = b"TZif Berlin";
    const OSLO: &[u8] = b"TZif Oslo, longer";

    /// A new, empty scratch directory named after the test, so that tests can run in parallel.
    fn scratch_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("java-runtime-time-zone-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes `bytes` to `path`, creating its directories.
    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_copied_localtime_is_named_after_the_zone_file_with_its_bytes() {
        let root = scratch_dir("copy");
        let zones = root.join("zoneinfo");
        write(&zones.join("Europe/Berlin"), BERLIN);
        write(&zones.join("Europe/Oslo"), OSLO);
        write(&zones.join("Europe/Prague"), PRAGUE);
        let localtime = root.join("localtime");
        write(&localtime, PRAGUE);
        assert_eq!(zone_of_localtime(&localtime, &zones), Some("Europe/Prague".to_string()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_skipped_entries_are_not_named_for_a_copy() {
        let root = scratch_dir("skipped");
        let zones = root.join("zoneinfo");
        write(&zones.join("posix/Europe/Prague"), PRAGUE);
        write(&zones.join("right/Europe/Prague"), PRAGUE);
        write(&zones.join("posixrules"), PRAGUE);
        write(&zones.join("localtime"), PRAGUE);
        let localtime = root.join("localtime");
        write(&localtime, PRAGUE);
        assert_eq!(zone_of_localtime(&localtime, &zones), None);
        // With the zone itself present, the copy under `posix` does not give the name.
        write(&zones.join("Europe/Prague"), PRAGUE);
        assert_eq!(zone_of_localtime(&localtime, &zones), Some("Europe/Prague".to_string()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_copy_matching_no_zone_file_names_no_zone() {
        let root = scratch_dir("no-match");
        let zones = root.join("zoneinfo");
        write(&zones.join("Europe/Prague"), PRAGUE);
        let localtime = root.join("localtime");
        write(&localtime, b"TZif Nowhere");
        assert_eq!(zone_of_localtime(&localtime, &zones), None);
        assert_eq!(zone_of_localtime(&root.join("missing"), &zones), None);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_localtime_link_names_the_zone_after_zoneinfo() {
        let root = scratch_dir("link");
        let zones = root.join("zoneinfo");
        write(&zones.join("Europe/Prague"), PRAGUE);
        let localtime = root.join("localtime");
        std::os::unix::fs::symlink(zones.join("Europe/Prague"), &localtime).unwrap();
        assert_eq!(zone_of_localtime(&localtime, &zones), Some("Europe/Prague".to_string()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_loop_of_links_in_the_zone_directory_ends_the_search() {
        let root = scratch_dir("loop");
        let zones = root.join("zoneinfo");
        write(&zones.join("Europe/Prague"), PRAGUE);
        std::os::unix::fs::symlink(&zones, zones.join("loop")).unwrap();
        let localtime = root.join("localtime");
        write(&localtime, b"TZif Nowhere");
        assert_eq!(zone_of_localtime(&localtime, &zones), None);
        write(&localtime, PRAGUE);
        assert_eq!(zone_of_localtime(&localtime, &zones), Some("Europe/Prague".to_string()));
        std::fs::remove_dir_all(&root).unwrap();
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
