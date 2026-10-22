//! The name service calls of `java.net` (`ast::Native::Net`): what the JVM's natives of
//! `InetAddress` and its implementations ask the C library, on the bytes of host names and
//! addresses, for the translator's `java.net.TranslatedNet`
//! (`java/overlay/java/net/TranslatedNet.java`), which makes the JDK's objects of them. None of
//! these opens a connection: they look names up (`getaddrinfo`) and addresses
//! (`getnameinfo`), name the host (`gethostname`),
//! list the addresses of the network interfaces (`getifaddrs`), and test whether the system
//! makes sockets of an address family, closing the one it makes at once, as the JVM does when
//! it starts. A failed lookup records its `getaddrinfo` error, which `net_last_error` gives.

use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_void};

thread_local! {
    /// The `getaddrinfo` error of this thread's last lookup, 0 where it succeeded.
    static LAST_ERROR: Cell<i32> = const { Cell::new(0) };
}

const AF_INET: i32 = 2;
const AF_INET6: i32 = 10;
const AF_UNSPEC: i32 = 0;
const SOCK_STREAM: i32 = 1;
const AI_CANONNAME: i32 = 2;
/// `EAI_NONAME`, for a name that C cannot take (one with a zero byte).
const EAI_NONAME: i32 = -2;
/// `NI_NAMEREQD`: `getnameinfo` fails where the address has no name, rather than giving the
/// address as text.
const NI_NAMEREQD: i32 = 8;
/// `NI_MAXHOST`, the longest host name, which the JVM's buffer holds with its terminator.
const NI_MAXHOST: usize = 1025;

/// C's `struct addrinfo` on Linux.
#[repr(C)]
#[allow(dead_code)]
struct AddrInfo {
    ai_flags: i32,
    ai_family: i32,
    ai_socktype: i32,
    ai_protocol: i32,
    ai_addrlen: u32,
    ai_addr: *mut SockAddr,
    ai_canonname: *mut c_char,
    ai_next: *mut AddrInfo,
}

/// C's `struct sockaddr`, whose family says which of the two below it is.
#[repr(C)]
#[allow(dead_code)]
struct SockAddr {
    sa_family: u16,
    sa_data: [u8; 14],
}

/// C's `struct sockaddr_in`.
#[repr(C)]
#[allow(dead_code)]
struct SockAddrIn {
    sin_family: u16,
    sin_port: u16,
    sin_addr: [u8; 4],
    sin_zero: [u8; 8],
}

/// C's `struct sockaddr_in6`.
#[repr(C)]
#[allow(dead_code)]
struct SockAddrIn6 {
    sin6_family: u16,
    sin6_port: u16,
    sin6_flowinfo: u32,
    sin6_addr: [u8; 16],
    sin6_scope_id: u32,
}

/// C's `struct ifaddrs` on Linux.
#[repr(C)]
#[allow(dead_code)]
struct IfAddrs {
    ifa_next: *mut IfAddrs,
    ifa_name: *mut c_char,
    ifa_flags: u32,
    ifa_addr: *mut SockAddr,
    ifa_netmask: *mut SockAddr,
    ifa_ifu: *mut SockAddr,
    ifa_data: *mut c_void,
}

unsafe extern "C" {
    fn getaddrinfo(
        node: *const c_char,
        service: *const c_char,
        hints: *const AddrInfo,
        res: *mut *mut AddrInfo,
    ) -> i32;
    fn freeaddrinfo(res: *mut AddrInfo);
    fn gai_strerror(code: i32) -> *const c_char;
    fn gethostname(name: *mut c_char, len: usize) -> i32;
    fn getnameinfo(
        addr: *const SockAddr,
        addrlen: u32,
        host: *mut c_char,
        hostlen: u32,
        serv: *mut c_char,
        servlen: u32,
        flags: i32,
    ) -> i32;
    fn getifaddrs(ifap: *mut *mut IfAddrs) -> i32;
    fn freeifaddrs(ifa: *mut IfAddrs);
    fn socket(domain: i32, kind: i32, protocol: i32) -> i32;
    fn close(fd: i32) -> i32;
}

fn to_bytes(text: &[u8]) -> Vec<i8> {
    text.iter().map(|&b| b as i8).collect()
}

fn from_bytes(bytes: &[i8]) -> Vec<u8> {
    bytes.iter().map(|&b| b as u8).collect()
}

/// An address of a socket address: its family's 4 or 16 bytes, and an IPv6 address's scope.
/// Safety: `addr` points to a socket address of the family it says.
unsafe fn address_of(addr: *const SockAddr) -> Option<(Vec<u8>, u32)> {
    if addr.is_null() {
        return None;
    }
    // Safety: the caller gives a valid socket address, whose family says its layout.
    unsafe {
        match i32::from((*addr).sa_family) {
            AF_INET => Some(((*addr.cast::<SockAddrIn>()).sin_addr.to_vec(), 0)),
            AF_INET6 => {
                let v6 = &*addr.cast::<SockAddrIn6>();
                Some((v6.sin6_addr.to_vec(), v6.sin6_scope_id))
            }
            _ => None,
        }
    }
}

/// `getaddrinfo(host)` of the addresses of `family` (4, 6, or 0 for both), as the JVM's
/// `lookupAllHostAddr` asks it: each address once, in the order given, each as its length (4 or
/// 16), its bytes, and for an IPv6 one its scope (4 bytes, high first). Empty, with the error
/// recorded, where the lookup fails.
pub fn net_lookup(host: &[i8], family: i32) -> Vec<i8> {
    let Ok(name) = CString::new(from_bytes(host)) else {
        LAST_ERROR.with(|e| e.set(EAI_NONAME));
        return Vec::new();
    };
    let hints = AddrInfo {
        ai_flags: AI_CANONNAME,
        ai_family: match family {
            4 => AF_INET,
            6 => AF_INET6,
            _ => AF_UNSPEC,
        },
        ai_socktype: 0,
        ai_protocol: 0,
        ai_addrlen: 0,
        ai_addr: std::ptr::null_mut(),
        ai_canonname: std::ptr::null_mut(),
        ai_next: std::ptr::null_mut(),
    };
    let mut res: *mut AddrInfo = std::ptr::null_mut();
    // Safety: the name is a C string, the hints are initialized, and `res` receives the list.
    let error = unsafe { getaddrinfo(name.as_ptr(), std::ptr::null(), &hints, &mut res) };
    LAST_ERROR.with(|e| e.set(error));
    if error != 0 {
        return Vec::new();
    }
    let mut seen: Vec<Vec<u8>> = Vec::new();
    let mut out = Vec::new();
    let mut at = res;
    while !at.is_null() {
        // Safety: `at` is an entry of the list that `getaddrinfo` gave, freed only below.
        let entry = unsafe { &*at };
        // Safety: an entry's address is of the family it says.
        if let Some((bytes, scope)) = unsafe { address_of(entry.ai_addr) }
            && !seen.contains(&bytes)
        {
            out.push(bytes.len() as i8);
            out.extend(bytes.iter().map(|&b| b as i8));
            if bytes.len() == 16 {
                out.extend(scope.to_be_bytes().iter().map(|&b| b as i8));
            }
            seen.push(bytes);
        }
        at = entry.ai_next;
    }
    // Safety: `res` is the list that `getaddrinfo` gave, not used after this.
    unsafe { freeaddrinfo(res) };
    out
}

/// The `getaddrinfo` error of this thread's last `net_lookup`, 0 where it succeeded.
pub fn net_last_error() -> i32 {
    LAST_ERROR.with(Cell::get)
}

/// `gai_strerror(code)`: the C library's text of a lookup's error.
pub fn net_error_text(code: i32) -> Vec<i8> {
    // Safety: `gai_strerror` returns a static C string, or null.
    let text = unsafe { gai_strerror(code) };
    if text.is_null() {
        return to_bytes(b"unknown error");
    }
    // Safety: a C string that stays valid.
    to_bytes(unsafe { CStr::from_ptr(text) }.to_bytes())
}

/// `getnameinfo` of an address of 4 or 16 bytes, as the JVM's `getHostByAddr` asks it: the name
/// that the system has for it, or empty where it has none (or the address is of another length),
/// with the error recorded as `net_lookup` records its own.
pub fn net_host_of(address: &[i8]) -> Vec<i8> {
    let bytes = from_bytes(address);
    let mut host = vec![0u8; NI_MAXHOST + 1];
    let error = match bytes.len() {
        4 => {
            let addr = SockAddrIn {
                sin_family: AF_INET as u16,
                sin_port: 0,
                sin_addr: bytes.clone().try_into().expect("four bytes"),
                sin_zero: [0; 8],
            };
            // Safety: the address is a `sockaddr_in` of its size, and the buffer has room for
            // `NI_MAXHOST` bytes and a terminator.
            unsafe {
                getnameinfo(
                    (&raw const addr).cast(),
                    size_of::<SockAddrIn>() as u32,
                    host.as_mut_ptr().cast(),
                    NI_MAXHOST as u32,
                    std::ptr::null_mut(),
                    0,
                    NI_NAMEREQD,
                )
            }
        }
        16 => {
            let addr = SockAddrIn6 {
                sin6_family: AF_INET6 as u16,
                sin6_port: 0,
                sin6_flowinfo: 0,
                sin6_addr: bytes.clone().try_into().expect("sixteen bytes"),
                sin6_scope_id: 0,
            };
            // Safety: as above, with a `sockaddr_in6`.
            unsafe {
                getnameinfo(
                    (&raw const addr).cast(),
                    size_of::<SockAddrIn6>() as u32,
                    host.as_mut_ptr().cast(),
                    NI_MAXHOST as u32,
                    std::ptr::null_mut(),
                    0,
                    NI_NAMEREQD,
                )
            }
        }
        _ => EAI_NONAME,
    };
    LAST_ERROR.with(|e| e.set(error));
    if error != 0 {
        return Vec::new();
    }
    let end = host.iter().position(|&b| b == 0).unwrap_or(NI_MAXHOST);
    to_bytes(&host[..end])
}

/// `gethostname()`: the name of this host, or `localhost` where the call fails, as the JVM's
/// `getLocalHostName` gives it.
pub fn net_hostname() -> Vec<i8> {
    let mut buffer = vec![0u8; NI_MAXHOST + 1];
    // Safety: the buffer has room for `NI_MAXHOST` bytes and a terminator.
    if unsafe { gethostname(buffer.as_mut_ptr().cast(), NI_MAXHOST + 1) } != 0 {
        return to_bytes(b"localhost");
    }
    buffer[NI_MAXHOST] = 0;
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(NI_MAXHOST);
    to_bytes(&buffer[..end])
}

/// Whether the system makes a stream socket of `family`, which it then closes.
fn makes_sockets(family: i32) -> bool {
    // Safety: `socket` takes plain numbers; a descriptor it gives is closed at once.
    unsafe {
        let fd = socket(family, SOCK_STREAM, 0);
        if fd < 0 {
            return false;
        }
        close(fd);
    }
    true
}

/// `InetAddress.isIPv4Available()`: whether the system makes IPv4 sockets.
pub fn net_ipv4_available() -> bool {
    makes_sockets(AF_INET)
}

/// `InetAddress.isIPv6Supported()`: whether the system makes IPv6 sockets and, as the JVM tests
/// on Linux, some interface has an IPv6 address (a line of `/proc/net/if_inet6`).
pub fn net_ipv6_supported() -> bool {
    makes_sockets(AF_INET6)
        && std::fs::read_to_string("/proc/net/if_inet6").is_ok_and(|lines| !lines.trim().is_empty())
}

/// `NetworkInterface.boundInetAddress0(address)`: whether one of the network interfaces has the
/// address, of its family, by its bytes; false where the interfaces cannot be listed.
pub fn net_bound(address: &[i8]) -> bool {
    let wanted = from_bytes(address);
    let mut list: *mut IfAddrs = std::ptr::null_mut();
    // Safety: `list` receives the interfaces' addresses, freed below.
    if unsafe { getifaddrs(&mut list) } != 0 {
        return false;
    }
    let mut found = false;
    let mut at = list;
    while !at.is_null() && !found {
        // Safety: `at` is an entry of the list that `getifaddrs` gave.
        let entry = unsafe { &*at };
        // Safety: an entry's address is null or of the family it says.
        found = unsafe { address_of(entry.ifa_addr) }.is_some_and(|(bytes, _)| bytes == wanted);
        at = entry.ifa_next;
    }
    // Safety: `list` is the list that `getifaddrs` gave, not used after this.
    unsafe { freeifaddrs(list) };
    found
}
