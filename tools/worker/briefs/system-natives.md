# System, Runtime and Thread natives; standard input and file natives in the runtime

Add the JVM services that programs call directly, and prepare the input/output natives that the
JDK's own I/O classes bottom out in. First read `lower/natives.rs`, the runtime's `natives.rs`
and `out.rs`, and README's paragraph on the native layer.

## Scope
- `System.getProperty(key)` and `getProperty(key, default)` for the keys whose values do not
  depend on the JVM: `line.separator`, `file.separator`, `path.separator`, `user.dir`,
  `user.home`, `user.name`, `java.io.tmpdir`, `os.name`, `os.arch` (Java's names: `Linux`,
  `amd64`), read at run time like Java does; unknown keys give null (the result is a nullable
  String: see how nullable strings work in README). Keys whose value is the JVM's own
  (`java.version`, `java.vendor`, ...) stay rejected with a clear message.
- `Runtime.getRuntime().totalMemory()/freeMemory()/maxMemory()` (plausible values; document
  that they are not the JVM's), `gc()` (no-op), `Runtime.halt` (exit without hooks),
  `Thread.sleep(ms)` and `sleep(ms, nanos)`, `Thread.onSpinWait()`, `Thread.yield()`.
- Standard input and files as runtime functions with unit tests, matching the natives of
  `java.io.FileInputStream` (`open0`, `read0`, `readBytes`, `available0`, `skip0`, `close0`),
  `java.io.FileOutputStream` (`open0`, `write`, `writeBytes`, `close0`) and
  `java.io.FileDescriptor` (`in`, `out`, `err`): a small file-descriptor table in the runtime,
  stdin read through a buffered reader shared with nothing else, stdout and stderr writing
  through the runtime's existing buffered `System.out` (read `out.rs`: a native write to fd 1
  must not reorder against `println!`). These cannot be used from translated programs yet: the
  JDK classes calling them need the object model that another agent is building. Recognise
  them by owner and name in `lower/natives.rs` as the other natives are, so they are ready
  when it lands, and test the runtime side with Rust unit tests (stdin through a test helper,
  files in a temporary directory).
- Do not touch object, class, nullness, borrowing, String or StringBuilder code (other agents
  are changing it now); keep edits in `lower.rs`'s `call` small and grouped.

## Deliverables
- System.getProperty for the JVM-independent keys, with null for unknown keys
- Runtime memory methods, gc, halt; Thread.sleep, onSpinWait, yield
- Runtime file-descriptor table with stdin, stdout, stderr and file natives, unit-tested
- The FileInputStream, FileOutputStream and FileDescriptor natives recognised in lower/natives.rs
- Corpus programs corpus/features/SystemProperties.java and corpus/features/RuntimeAndSleep.java
- README native-layer paragraph updated
