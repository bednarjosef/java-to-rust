//! The system calls of `sun.nio.ch`'s sockets, pipes and selectors (`ast::Native::Socket`), for
//! the translator's `sun.nio.ch.TranslatedSockets` (`java/overlay/sun/nio/ch/TranslatedSockets.java`),
//! whose methods the calls of the JVM's natives of `Net`, `IOUtil`, `SocketDispatcher`,
//! `UnixDispatcher`, `EPoll`, `EventFD` and `NativeThread` are redirected to
//! (`program::SOCKET_NATIVES`). Each descriptor is one of `fd.rs`, holding the system's
//! descriptor. An address is its bytes (4 for IPv4, 16 for IPv6) and a port.
//!
//! None of these raises an exception: each records the `errno` of its failure, or 0, which
//! `socket_last_error` gives, and the Java side throws what the JVM's C code throws for it. The
//! reads, writes, accepts and connects answer the JVM's `IOStatus` codes: the count, `EOF` (-1),
//! `UNAVAILABLE` (-2) where a non-blocking call would block, `INTERRUPTED` (-3), and `THROWN`
//! (-5) where the JVM throws.
//!
//! A blocking call holds no lock: it runs on the system's descriptor, which stays open until the
//! channel closes it, and a channel closes only once no thread is in a call on it. A thread that
//! another thread's close must wake is woken by `socket_pre_close`, which shuts the socket down
//! both ways, where the JVM signals the thread.

use crate::fd;
use crate::native_memory::{get_native, read_native, write_native};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::os::fd::{FromRawFd, OwnedFd};

thread_local! {
    /// The `errno` of this thread's last call, 0 where it succeeded.
    static LAST_ERROR: Cell<i32> = const { Cell::new(0) };
    /// The address and port of the peer of this thread's last `socket_accept`.
    static ACCEPTED: RefCell<Vec<i8>> = const { RefCell::new(Vec::new()) };
}

const AF_INET: i32 = 2;
const AF_INET6: i32 = 10;
const SOCK_STREAM: i32 = 1;
const SOCK_DGRAM: i32 = 2;
const SOCK_CLOEXEC: i32 = 0o2000000;
const SOL_SOCKET: i32 = 1;
const SO_REUSEADDR: i32 = 2;
const SO_ERROR: i32 = 4;
const SO_LINGER: i32 = 13;
const IPPROTO_IPV6: i32 = 41;
const IPV6_V6ONLY: i32 = 26;
const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;
const O_CLOEXEC: i32 = 0o2000000;
const FIONREAD: u64 = 0x541B;
const SHUT_RDWR: i32 = 2;
const POLLOUT: i16 = 4;
const POLLERR: i16 = 8;
const POLLHUP: i16 = 16;
const RLIMIT_NOFILE: i32 = 7;
const EFD_CLOEXEC: i32 = 0o2000000;

const EBADF: i32 = 9;
const EINTR: i32 = 4;
const EAGAIN: i32 = 11;
const EINPROGRESS: i32 = 115;

/// `IOStatus` of `sun.nio.ch`.
const EOF: i32 = -1;
const UNAVAILABLE: i32 = -2;
const INTERRUPTED: i32 = -3;
const THROWN: i32 = -5;

/// C's `struct sockaddr_in6`, which holds an IPv4 address too: `sockaddr_in` has the family and
/// port at the same places.
#[repr(C)]
#[derive(Default)]
struct SockAddr {
    family: u16,
    port: u16,
    /// IPv4: the address in the first 4 bytes. IPv6: the flow information.
    flow: [u8; 4],
    addr: [u8; 16],
    scope: u32,
}

/// C's `struct pollfd`.
#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

/// C's `struct epoll_event`, packed on x86-64 as the kernel has it.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EpollEvent {
    events: u32,
    data: u64,
}

unsafe extern "C" {
    fn socket(domain: i32, ty: i32, protocol: i32) -> i32;
    fn bind(fd: i32, addr: *const SockAddr, len: u32) -> i32;
    fn listen(fd: i32, backlog: i32) -> i32;
    fn accept4(fd: i32, addr: *mut SockAddr, len: *mut u32, flags: i32) -> i32;
    fn connect(fd: i32, addr: *const SockAddr, len: u32) -> i32;
    fn getsockname(fd: i32, addr: *mut SockAddr, len: *mut u32) -> i32;
    fn getpeername(fd: i32, addr: *mut SockAddr, len: *mut u32) -> i32;
    fn getsockopt(fd: i32, level: i32, name: i32, value: *mut c_void, len: *mut u32) -> i32;
    fn setsockopt(fd: i32, level: i32, name: i32, value: *const c_void, len: u32) -> i32;
    fn shutdown(fd: i32, how: i32) -> i32;
    fn poll(fds: *mut PollFd, n: u64, timeout: i32) -> i32;
    fn read(fd: i32, buf: *mut c_void, n: usize) -> isize;
    fn write(fd: i32, buf: *const c_void, n: usize) -> isize;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn pipe2(fds: *mut i32, flags: i32) -> i32;
    fn getrlimit(resource: i32, limit: *mut [u64; 2]) -> i32;
    fn epoll_create1(flags: i32) -> i32;
    fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut EpollEvent) -> i32;
    fn epoll_wait(epfd: i32, events: *mut EpollEvent, max: i32, timeout: i32) -> i32;
    fn eventfd(initval: u32, flags: i32) -> i32;
    fn __errno_location() -> *mut i32;
}

fn errno() -> i32 {
    // Safety: the thread's own `errno`.
    unsafe { *__errno_location() }
}

fn record(error: i32) {
    LAST_ERROR.with(|e| e.set(error));
}

/// Records the outcome of a call that answers -1 where it fails, and answers whether it did not.
fn checked(result: i32) -> bool {
    record(if result < 0 { errno() } else { 0 });
    result >= 0
}

/// The system's descriptor of `fd`, or `None` with `EBADF` recorded.
fn system(fd: i32) -> Option<i32> {
    match fd::system_descriptor(fd) {
        Ok(system) => Some(system),
        Err(_) => {
            record(EBADF);
            None
        }
    }
}

/// A new descriptor of the table for the system's descriptor `system`, or -1 where the call
/// that made it failed (with its `errno` recorded).
fn adopt(system: i32) -> i32 {
    if !checked(system) {
        return -1;
    }
    // Safety: the call just made `system`, and nothing else owns it.
    fd::insert_system(unsafe { OwnedFd::from_raw_fd(system) })
}

/// The status a read, write, accept or connect answers for its `errno`, as the JVM's
/// `convertReturnVal` and `handleSocketError` do.
fn status(error: i32) -> i32 {
    record(error);
    match error {
        EAGAIN => UNAVAILABLE,
        EINTR => INTERRUPTED,
        _ => THROWN,
    }
}

/// The address of `bytes` and `port` for a socket of `ipv6`: an IPv4 address on an IPv6 socket
/// is mapped (`::ffff:a.b.c.d`), as the JVM's `NET_InetAddressToSockaddr` does.
fn address_of(ipv6: bool, bytes: &[i8], port: i32) -> (SockAddr, u32) {
    let bytes: Vec<u8> = bytes.iter().map(|&b| b as u8).collect();
    let mut addr = SockAddr { port: (port as u16).to_be(), ..SockAddr::default() };
    if ipv6 {
        addr.family = AF_INET6 as u16;
        if bytes.len() == 4 {
            addr.addr[10] = 0xff;
            addr.addr[11] = 0xff;
            addr.addr[12..].copy_from_slice(&bytes);
        } else {
            addr.addr.copy_from_slice(&bytes[..16]);
        }
        (addr, std::mem::size_of::<SockAddr>() as u32)
    } else {
        addr.family = AF_INET as u16;
        addr.flow.copy_from_slice(&bytes[..4]);
        (addr, 16)
    }
}

/// The bytes of an address the system gave and its port, two bytes big-endian after them; a
/// mapped IPv4 address is the IPv4 one, as the JVM's `NET_SockaddrToInetAddress` gives it.
fn bytes_of(addr: &SockAddr) -> Vec<i8> {
    let mut bytes: Vec<u8> = if addr.family == AF_INET as u16 {
        addr.flow.to_vec()
    } else if addr.addr[..10].iter().all(|&b| b == 0) && addr.addr[10] == 0xff && addr.addr[11] == 0xff {
        addr.addr[12..].to_vec()
    } else {
        addr.addr.to_vec()
    };
    bytes.extend(u16::from_be(addr.port).to_be_bytes());
    bytes.into_iter().map(|b| b as i8).collect()
}

/// The `errno` of this thread's last call, 0 where it succeeded.
pub fn socket_last_error() -> i32 {
    LAST_ERROR.with(Cell::get)
}

/// `Net.isIPv6Available0()`: whether the system makes IPv6 sockets.
pub fn socket_ipv6_available() -> bool {
    // Safety: plain system calls on a descriptor of our own.
    let fd = unsafe { socket(AF_INET6, SOCK_STREAM | SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return false;
    }
    drop(unsafe { OwnedFd::from_raw_fd(fd) });
    true
}

/// `Net.socket0(preferIPv6, stream, reuse, fastLoopback)`: a socket, IPv6 and taking IPv4 too
/// where `ipv6`, with `SO_REUSEADDR` where `reuse`; -1 where it fails.
pub fn socket_open(ipv6: bool, stream: bool, reuse: bool) -> i32 {
    let domain = if ipv6 { AF_INET6 } else { AF_INET };
    let ty = if stream { SOCK_STREAM } else { SOCK_DGRAM };
    // Safety: plain system calls.
    let fd = adopt(unsafe { socket(domain, ty | SOCK_CLOEXEC, 0) });
    if fd < 0 {
        return -1;
    }
    let system = fd::system_descriptor(fd).unwrap_or(-1);
    let zero = 0i32;
    let one = 1i32;
    if ipv6
        && !checked(unsafe { setsockopt(system, IPPROTO_IPV6, IPV6_V6ONLY, (&zero as *const i32).cast(), 4) })
        || reuse
            && !checked(unsafe {
                setsockopt(system, SOL_SOCKET, SO_REUSEADDR, (&one as *const i32).cast(), 4)
            })
    {
        let error = socket_last_error();
        fd::close_descriptor(fd);
        record(error);
        return -1;
    }
    record(0);
    fd
}

/// `Net.bind0`: 0, or -1 where it fails.
pub fn socket_bind(fd: i32, ipv6: bool, address: &[i8], port: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let (addr, len) = address_of(ipv6, address, port);
    // Safety: `addr` is a socket address of `len` bytes.
    if checked(unsafe { bind(system, &addr, len) }) { 0 } else { -1 }
}

/// `Net.listen`: 0, or -1 where it fails.
pub fn socket_listen(fd: i32, backlog: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    if checked(unsafe { listen(system, backlog) }) { 0 } else { -1 }
}

/// `Net.connect0`: 1 where connected, `UNAVAILABLE` where a non-blocking connect goes on,
/// `INTERRUPTED`, or `THROWN`.
pub fn socket_connect(fd: i32, ipv6: bool, address: &[i8], port: i32) -> i32 {
    let Some(system) = system(fd) else { return THROWN };
    let (addr, len) = address_of(ipv6, address, port);
    // Safety: `addr` is a socket address of `len` bytes.
    if unsafe { connect(system, &addr, len) } == 0 {
        record(0);
        return 1;
    }
    match errno() {
        EINPROGRESS => {
            record(0);
            UNAVAILABLE
        }
        error => status(error),
    }
}

/// `Net.accept`: the descriptor of the connection accepted, whose peer `socket_accepted_peer`
/// then gives; or `UNAVAILABLE`, `INTERRUPTED` or `THROWN`.
pub fn socket_accept(fd: i32) -> i32 {
    let Some(system) = system(fd) else { return THROWN };
    let mut addr = SockAddr::default();
    let mut len = std::mem::size_of::<SockAddr>() as u32;
    // Safety: `addr` holds `len` bytes.
    let accepted = unsafe { accept4(system, &mut addr, &mut len, SOCK_CLOEXEC) };
    if accepted < 0 {
        return status(errno());
    }
    ACCEPTED.with(|a| *a.borrow_mut() = bytes_of(&addr));
    adopt(accepted)
}

/// The address and port of the peer of this thread's last accepted connection.
pub fn socket_accepted_peer() -> Vec<i8> {
    ACCEPTED.with(|a| a.borrow().clone())
}

/// `getsockname` (`Net.localInetAddress`, `localPort`): the address's bytes and its port, or
/// none where it fails.
pub fn socket_local_address(fd: i32) -> Vec<i8> {
    named(fd, getsockname)
}

/// `getpeername` (`Net.remoteInetAddress`, `remotePort`), as `socket_local_address`.
pub fn socket_remote_address(fd: i32) -> Vec<i8> {
    named(fd, getpeername)
}

fn named(fd: i32, call: unsafe extern "C" fn(i32, *mut SockAddr, *mut u32) -> i32) -> Vec<i8> {
    let Some(system) = system(fd) else { return Vec::new() };
    let mut addr = SockAddr::default();
    let mut len = std::mem::size_of::<SockAddr>() as u32;
    // Safety: `addr` holds `len` bytes.
    if checked(unsafe { call(system, &mut addr, &mut len) }) { bytes_of(&addr) } else { Vec::new() }
}

/// `Net.getIntOption0`: the option's value, `SO_LINGER`'s seconds or -1 where it is off.
pub fn socket_get_option(fd: i32, level: i32, option: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let mut value = [0i32; 2];
    let mut len = if level == SOL_SOCKET && option == SO_LINGER { 8 } else { 4 };
    // Safety: `value` holds `len` bytes.
    if !checked(unsafe { getsockopt(system, level, option, value.as_mut_ptr().cast(), &mut len) }) {
        return -1;
    }
    if level == SOL_SOCKET && option == SO_LINGER {
        return if value[0] != 0 { value[1] } else { -1 };
    }
    value[0]
}

/// `Net.setIntOption0`: 0, or -1 where it fails. `SO_LINGER` of a negative value turns it off.
pub fn socket_set_option(fd: i32, level: i32, option: i32, arg: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let value = if level == SOL_SOCKET && option == SO_LINGER {
        if arg >= 0 { [1, arg] } else { [0, 0] }
    } else {
        [arg, 0]
    };
    let len = if level == SOL_SOCKET && option == SO_LINGER { 8 } else { 4 };
    // Safety: `value` holds `len` bytes.
    if checked(unsafe { setsockopt(system, level, option, value.as_ptr().cast(), len) }) { 0 } else { -1 }
}

/// `Net.shutdown`: 0, or -1 where it fails.
pub fn socket_shutdown(fd: i32, how: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    if checked(unsafe { shutdown(system, how) }) { 0 } else { -1 }
}

/// `Net.poll`: the events that happened within `timeout` milliseconds (-1: no limit), 0 where
/// none did or the wait was interrupted, -1 where it fails.
pub fn socket_poll(fd: i32, events: i32, timeout: i64) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let mut pfd = PollFd { fd: system, events: events as i16, revents: 0 };
    let timeout = timeout.clamp(-1, i32::MAX as i64) as i32;
    // Safety: one `pollfd`.
    let n = unsafe { poll(&mut pfd, 1, timeout) };
    if n >= 0 {
        record(0);
        return pfd.revents as i32;
    }
    match errno() {
        EINTR => {
            record(0);
            0
        }
        error => {
            record(error);
            -1
        }
    }
}

/// `Net.pollConnect`: 1 where the connection that `socket_connect` began is made, 0 where it
/// is not yet, -1 where it failed (with its error recorded).
pub fn socket_poll_connect(fd: i32, timeout: i64) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let mut pfd = PollFd { fd: system, events: POLLOUT, revents: 0 };
    let timeout = timeout.clamp(-1, i32::MAX as i64) as i32;
    let n = unsafe { poll(&mut pfd, 1, timeout) };
    if n < 0 {
        return if errno() == EINTR {
            record(0);
            0
        } else {
            record(errno());
            -1
        };
    }
    if n == 0 || pfd.revents & (POLLOUT | POLLERR | POLLHUP) == 0 {
        record(0);
        return 0;
    }
    let mut error = 0i32;
    let mut len = 4u32;
    if !checked(unsafe {
        getsockopt(system, SOL_SOCKET, SO_ERROR, (&mut error as *mut i32).cast(), &mut len)
    }) {
        return -1;
    }
    record(error);
    if error == 0 { 1 } else { -1 }
}

/// `Net.available`: the bytes ready to read, or -1 where it fails.
pub fn socket_available(fd: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let mut n = 0i32;
    if checked(unsafe { ioctl(system, FIONREAD, &mut n as *mut i32) }) { n } else { -1 }
}

/// `SocketDispatcher.read0` (and `IOUtil`'s reads of a pipe): up to `len` bytes into the block
/// at `address`; the count, `EOF` where the peer closed its side, or another status.
pub fn socket_read(fd: i32, address: i64, len: i32) -> i32 {
    let Some(system) = system(fd) else { return THROWN };
    let mut bytes = vec![0u8; len.max(0) as usize];
    // Safety: `bytes` holds `len` bytes.
    let n = unsafe { read(system, bytes.as_mut_ptr().cast(), bytes.len()) };
    if n < 0 {
        return status(errno());
    }
    record(0);
    if n == 0 && len > 0 {
        return EOF;
    }
    write_native(address, &bytes[..n as usize]);
    n as i32
}

/// `SocketDispatcher.write0`: up to `len` bytes from the block at `address`; the count, or a
/// status.
pub fn socket_write(fd: i32, address: i64, len: i32) -> i32 {
    let Some(system) = system(fd) else { return THROWN };
    let bytes = if len > 0 { read_native(address, len as usize) } else { Vec::new() };
    // Safety: `bytes` holds its length.
    let n = unsafe { write(system, bytes.as_ptr().cast(), bytes.len()) };
    if n < 0 {
        return status(errno());
    }
    record(0);
    n as i32
}

/// The blocks of the `len` C `struct iovec`s at `address` (`IOVecWrapper`'s): each block's
/// address and length.
fn io_vectors(address: i64, len: i32) -> Vec<(i64, usize)> {
    (0..len.max(0) as i64)
        .map(|i| {
            let base = get_native::<i64>(address + 16 * i);
            let size = get_native::<i64>(address + 16 * i + 8);
            (base, size.max(0) as usize)
        })
        .collect()
}

/// `SocketDispatcher.readv0`: one read into the `len` blocks that the `struct iovec`s at
/// `address` name, filled in order; the count, `EOF`, or a status, as `socket_read`.
pub fn socket_readv(fd: i32, address: i64, len: i32) -> i64 {
    let Some(system) = system(fd) else { return THROWN as i64 };
    let blocks = io_vectors(address, len);
    let total: usize = blocks.iter().map(|(_, size)| size).sum();
    let mut bytes = vec![0u8; total];
    // Safety: `bytes` holds `total` bytes.
    let n = unsafe { read(system, bytes.as_mut_ptr().cast(), total) };
    if n < 0 {
        return status(errno()) as i64;
    }
    record(0);
    if n == 0 && total > 0 {
        return EOF as i64;
    }
    let mut rest = &bytes[..n as usize];
    for (base, size) in blocks {
        let take = size.min(rest.len());
        write_native(base, &rest[..take]);
        rest = &rest[take..];
    }
    n as i64
}

/// `SocketDispatcher.writev0`: one write of the `len` blocks that the `struct iovec`s at
/// `address` name, in order; the count, or a status.
pub fn socket_writev(fd: i32, address: i64, len: i32) -> i64 {
    let Some(system) = system(fd) else { return THROWN as i64 };
    let mut bytes = Vec::new();
    for (base, size) in io_vectors(address, len) {
        if size > 0 {
            bytes.extend(read_native(base, size));
        }
    }
    // Safety: `bytes` holds its length.
    let n = unsafe { write(system, bytes.as_ptr().cast(), bytes.len()) };
    if n < 0 {
        return status(errno()) as i64;
    }
    record(0);
    n as i64
}

/// `UnixDispatcher.close0`: closes the descriptor.
pub fn socket_close(fd: i32) {
    record(0);
    fd::close_descriptor(fd);
}

/// `UnixDispatcher.preClose0`: shuts a socket down both ways, so that a thread blocked on it
/// returns before the close; does nothing for a pipe or the like.
pub fn socket_pre_close(fd: i32) {
    if let Some(system) = system(fd) {
        unsafe { shutdown(system, SHUT_RDWR) };
    }
    record(0);
}

/// `IOUtil.configureBlocking`: 0, or -1 where it fails.
pub fn socket_configure_blocking(fd: i32, blocking: bool) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let flags = unsafe { fcntl(system, F_GETFL) };
    if !checked(flags) {
        return -1;
    }
    let flags = if blocking { flags & !O_NONBLOCK } else { flags | O_NONBLOCK };
    if checked(unsafe { fcntl(system, F_SETFL, flags) }) { 0 } else { -1 }
}

/// `IOUtil.fdLimit`: the soft limit of open descriptors.
pub fn socket_fd_limit() -> i32 {
    let mut limit = [0u64; 2];
    if unsafe { getrlimit(RLIMIT_NOFILE, &mut limit) } < 0 {
        return i32::MAX;
    }
    limit[0].min(i32::MAX as u64) as i32
}

/// `IOUtil.makePipe`: the read end in the high 32 bits, the write end in the low ones; -1 where
/// it fails.
pub fn socket_make_pipe(blocking: bool) -> i64 {
    let mut fds = [0i32; 2];
    let flags = O_CLOEXEC | if blocking { 0 } else { O_NONBLOCK };
    if !checked(unsafe { pipe2(fds.as_mut_ptr(), flags) }) {
        return -1;
    }
    let read_end = adopt(fds[0]);
    let write_end = adopt(fds[1]);
    ((read_end as i64) << 32) | write_end as i64
}

/// `IOUtil.drain`: reads what the descriptor holds until a read would block; whether it read
/// any. -1 where it fails.
pub fn socket_drain(fd: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let mut buf = [0u8; 16];
    let mut any = false;
    loop {
        let n = unsafe { read(system, buf.as_mut_ptr().cast(), buf.len()) };
        if n > 0 {
            any = true;
            if (n as usize) < buf.len() {
                break;
            }
        } else if n == 0 || errno() == EAGAIN {
            break;
        } else if errno() != EINTR {
            record(errno());
            return -1;
        }
    }
    record(0);
    any as i32
}

/// `IOUtil.write1`: writes one byte; 1, `UNAVAILABLE`, `INTERRUPTED` or `THROWN`.
pub fn socket_write1(fd: i32, b: i32) -> i32 {
    let Some(system) = system(fd) else { return THROWN };
    let byte = b as u8;
    let n = unsafe { write(system, (&byte as *const u8).cast(), 1) };
    if n < 0 {
        return status(errno());
    }
    record(0);
    n as i32
}

/// `EPoll.create`: an `epoll` instance, or -1 where it fails.
pub fn epoll_open() -> i32 {
    adopt(unsafe { epoll_create1(O_CLOEXEC) })
}

/// `EPoll.ctl`: 0, or the `errno` where it fails, as the JVM's answers. The event's data is the
/// table's descriptor `fd`, which `EPoll.getDescriptor` reads back.
pub fn epoll_control(epfd: i32, op: i32, fd: i32, events: i32) -> i32 {
    let (Some(epoll), Some(system)) = (system(epfd), system(fd)) else { return EBADF };
    let mut event = EpollEvent { events: events as u32, data: fd as u64 };
    if unsafe { epoll_ctl(epoll, op, system, &mut event) } < 0 { errno() } else { 0 }
}

/// `EPoll.wait`: waits up to `timeout` milliseconds (-1: no limit) for at most `max` events,
/// written at `address` as the kernel's `struct epoll_event`s: their number, `INTERRUPTED`
/// where the wait was interrupted, or -1 where it fails.
pub fn epoll_wait_events(epfd: i32, address: i64, max: i32, timeout: i32) -> i32 {
    let Some(epoll) = system(epfd) else { return -1 };
    let mut events = vec![EpollEvent { events: 0, data: 0 }; max.max(1) as usize];
    let n = unsafe { epoll_wait(epoll, events.as_mut_ptr(), max, timeout) };
    if n < 0 {
        return if errno() == EINTR {
            record(0);
            INTERRUPTED
        } else {
            record(errno());
            -1
        };
    }
    record(0);
    let size = std::mem::size_of::<EpollEvent>();
    // Safety: the events are plain bytes.
    let bytes = unsafe { std::slice::from_raw_parts(events.as_ptr().cast::<u8>(), n as usize * size) };
    if n > 0 {
        write_native(address, bytes);
    }
    n
}

/// `EventFD.eventfd0`: an event counter, or -1 where it fails.
pub fn event_fd_open() -> i32 {
    adopt(unsafe { eventfd(0, EFD_CLOEXEC) })
}

/// `EventFD.set0`: adds 1 to the counter; 0, or -1 where it fails.
pub fn event_fd_set(fd: i32) -> i32 {
    let Some(system) = system(fd) else { return -1 };
    let one = 1u64.to_ne_bytes();
    if unsafe { write(system, one.as_ptr().cast(), 8) } < 0 {
        record(errno());
        return -1;
    }
    record(0);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_memory::{allocate_native, free_native};

    const LOOPBACK: [i8; 4] = [127, 0, 0, 1];
    const EPOLLIN: i32 = 1;
    const EPOLL_CTL_ADD: i32 = 1;

    #[test]
    fn a_server_and_a_client_on_localhost_exchange_bytes_and_epoll_sees_them() {
        let server = socket_open(false, true, true);
        assert!(server >= 0);
        assert_eq!(socket_bind(server, false, &LOOPBACK, 0), 0);
        assert_eq!(socket_listen(server, 50), 0);
        let local = socket_local_address(server);
        assert_eq!(&local[..4], &LOOPBACK);
        let port = u16::from_be_bytes([local[4] as u8, local[5] as u8]) as i32;
        assert!(port > 0);

        // An IPv6 socket reaches the IPv4 server through the mapped address.
        let client = socket_open(true, true, false);
        assert_eq!(socket_connect(client, true, &LOOPBACK, port), 1);
        let accepted = socket_accept(server);
        assert!(accepted >= 0);
        assert_eq!(&socket_accepted_peer()[..4], &LOOPBACK);
        assert_eq!(&socket_remote_address(client)[4..], &local[4..]);

        let epoll = epoll_open();
        assert_eq!(epoll_control(epoll, EPOLL_CTL_ADD, accepted, EPOLLIN), 0);
        let events = allocate_native(12 * 4);
        assert_eq!(epoll_wait_events(epoll, events, 4, 0), 0);

        let block = allocate_native(16);
        write_native(block, &[1, 2, 3, 4, 5]);
        assert_eq!(socket_write(client, block, 5), 5);
        assert_eq!(epoll_wait_events(epoll, events, 4, 1000), 1);
        assert_eq!(read_native(events + 4, 8), (accepted as u64).to_ne_bytes());
        assert_eq!(socket_available(accepted), 5);
        write_native(block, &[0; 16]);
        assert_eq!(socket_read(accepted, block, 16), 5);
        assert_eq!(read_native(block, 5), [1, 2, 3, 4, 5]);

        assert_eq!(socket_configure_blocking(accepted, false), 0);
        assert_eq!(socket_read(accepted, block, 16), UNAVAILABLE);
        socket_close(client);
        assert_eq!(socket_read(accepted, block, 16), EOF);

        for fd in [accepted, server, epoll] {
            socket_close(fd);
        }
        assert_eq!(socket_read(accepted, block, 16), THROWN);
        assert_eq!(socket_last_error(), EBADF);
        free_native(block);
        free_native(events);
    }

    #[test]
    fn a_refused_connection_records_its_error() {
        let server = socket_open(false, true, true);
        assert_eq!(socket_bind(server, false, &LOOPBACK, 0), 0);
        let port = {
            let local = socket_local_address(server);
            u16::from_be_bytes([local[4] as u8, local[5] as u8]) as i32
        };
        // Bound but not listening: nothing accepts the connection.
        let client = socket_open(false, true, false);
        assert_eq!(socket_connect(client, false, &LOOPBACK, port), THROWN);
        assert_eq!(socket_last_error(), 111);
        socket_close(client);
        socket_close(server);
    }

    #[test]
    fn gathered_writes_and_scattered_reads_keep_the_blocks_in_order() {
        let pipe = socket_make_pipe(true);
        let (read_end, write_end) = ((pipe >> 32) as i32, pipe as i32);
        let first = allocate_native(3);
        let second = allocate_native(4);
        write_native(first, &[1, 2, 3]);
        write_native(second, &[4, 5, 6, 7]);
        let vectors = allocate_native(32);
        crate::native_memory::put_native(vectors, first);
        crate::native_memory::put_native(vectors + 8, 3i64);
        crate::native_memory::put_native(vectors + 16, second);
        crate::native_memory::put_native(vectors + 24, 4i64);
        assert_eq!(socket_writev(write_end, vectors, 2), 7);

        write_native(first, &[0; 3]);
        write_native(second, &[0; 4]);
        assert_eq!(socket_readv(read_end, vectors, 2), 7);
        assert_eq!(read_native(first, 3), [1, 2, 3]);
        assert_eq!(read_native(second, 4), [4, 5, 6, 7]);
        socket_close(write_end);
        assert_eq!(socket_readv(read_end, vectors, 2), EOF as i64);
        socket_close(read_end);
        for block in [first, second, vectors] {
            free_native(block);
        }
    }

    #[test]
    fn a_pipe_and_an_event_counter_drain() {
        let pipe = socket_make_pipe(false);
        let (read_end, write_end) = ((pipe >> 32) as i32, pipe as i32);
        assert_eq!(socket_drain(read_end), 0);
        assert_eq!(socket_write1(write_end, 7), 1);
        assert_eq!(socket_drain(read_end), 1);
        let counter = event_fd_open();
        assert_eq!(socket_configure_blocking(counter, false), 0);
        assert_eq!(event_fd_set(counter), 0);
        assert_eq!(socket_drain(counter), 1);
        assert_eq!(socket_drain(counter), 0);
        for fd in [read_end, write_end, counter] {
            socket_close(fd);
        }
    }
}
