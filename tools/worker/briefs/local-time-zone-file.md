# The system time zone when /etc/localtime is a copied file

`TimeZone.getSystemTimeZoneID` is a runtime function (`crates/runtime/src/time_zone.rs`). It
follows Java's Linux logic for the `TZ` variable, `/etc/timezone` and an `/etc/localtime`
symbolic link, but not the last case: when `/etc/localtime` is a regular file (a copy, as on some
distributions and in containers), Java searches the zone directory (`/usr/share/zoneinfo`) for a
file with the same size and the same bytes and uses its path relative to that directory as the
zone's name. Without it, the translation names a different zone from Java on such machines.

First read `time_zone.rs` and its tests, then find Java's behaviour: the JDK's Linux native
`TimeZone_md.c` is not in the sources here, so work from its documented behaviour:
- `/etc/localtime` is a regular file: walk `/usr/share/zoneinfo` recursively (skip the entries
  `posixrules`, `localtime`, and directories named `posix` and `right`, as the JDK does; check
  the JDK's own list in its documentation or source if you can find it, and say in your report
  where your list came from), and return the first file whose size and bytes equal
  `/etc/localtime`'s;
- no match: return null, so that Java's code then falls back to the GMT offset.

## Scope
- That search, in `time_zone.rs`, with the zone directory as a parameter so tests can use a
  temporary directory with a few fake zone files (including a `posix` copy that must be
  skipped).
- Nothing in the translator needs to change.

## Deliverables
- The copied-file case of getSystemTimeZoneID, with runtime unit tests on a temporary zone
  directory (match found, skipped directories, no match)

## How to test
- Build and test through the build-slot limiter, which keeps the machine from running out of
  memory: prefix every cargo command with
  `<scratchpad>/build-slot.sh`,
  for example `.../build-slot.sh cargo test -p java-runtime time_zone`.
- Test only what you touch while working; run `scripts/check.sh` once at the end, through the
  limiter.
- Change only `crates/runtime/src/time_zone.rs` (and its tests).
