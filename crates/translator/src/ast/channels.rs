//! The natives of `sun.nio.ch` that the runtime implements for `FileChannelImpl`, which javac
//! reads and writes files through (`java_runtime::file_channel`).

/// A native of `sun.nio.ch` that the runtime implements (`Native::Channel`): its class and name,
/// and the runtime's function that answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChannelCall {
    pub class: &'static str,
    pub name: &'static str,
    /// The runtime's function. It takes the arguments as Java's native does, a `FileDescriptor`
    /// as its descriptor number.
    pub function: &'static str,
    /// Whether the function returns a `Result` whose failure is raised as an `IOException`.
    pub fails: bool,
}

/// `UnixFileDispatcherImpl`, whose natives take a `FileDescriptor` first.
pub const UNIX_FILE_DISPATCHER: &str = "sun.nio.ch.UnixFileDispatcherImpl";

/// The natives of `sun.nio.ch` that the runtime implements, by the class and name Java gives them.
/// `setDirect0` fails with an `IOException` (direct I/O is not supported), as its caller expects.
/// `release0` releases file locks, which `lock0` takes and the runtime does not have, so it is
/// never called with a lock held and does nothing.
pub const CHANNEL_CALLS: &[ChannelCall] = &[
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "read0", function: "channel_read", fails: true },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "pread0", function: "channel_pread", fails: true },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "write0", function: "channel_write", fails: true },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "pwrite0", function: "channel_pwrite", fails: true },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "seek0", function: "channel_seek", fails: true },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "size0", function: "channel_size", fails: true },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "truncate0", function: "channel_truncate", fails: true },
    ChannelCall {
        class: UNIX_FILE_DISPATCHER,
        name: "available0",
        function: "channel_available",
        fails: true,
    },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "isOther0", function: "channel_is_other", fails: true },
    ChannelCall {
        class: UNIX_FILE_DISPATCHER,
        name: "setDirect0",
        function: "channel_set_direct",
        fails: true,
    },
    ChannelCall { class: UNIX_FILE_DISPATCHER, name: "release0", function: "channel_release", fails: false },
    ChannelCall {
        class: "sun.nio.ch.NativeThread",
        name: "current0",
        function: "native_thread_current",
        fails: false,
    },
    ChannelCall {
        class: "sun.nio.ch.NativeThread",
        name: "signal0",
        function: "native_thread_signal",
        fails: false,
    },
    ChannelCall { class: "sun.nio.ch.IOUtil", name: "iovMax", function: "io_iov_max", fails: false },
    ChannelCall { class: "sun.nio.ch.IOUtil", name: "writevMax", function: "io_writev_max", fails: false },
];

/// The `sun.nio.ch` native that runtime implements by `CHANNEL_CALLS`, if it is one.
pub fn channel_call(class: &str, name: &str) -> Option<&'static ChannelCall> {
    CHANNEL_CALLS.iter().find(|call| call.class == class && call.name == name)
}

/// The natives of `sun.nio.ch` that do nothing in a translated program: `FileDispatcherImpl`'s
/// `init0()`, which the JVM uses to look up its methods, and `NativeThread.init()`.
pub fn channel_nothing(class: &str, name: &str) -> bool {
    matches!((class, name), ("sun.nio.ch.FileDispatcherImpl", "init0") | ("sun.nio.ch.NativeThread", "init"))
}
