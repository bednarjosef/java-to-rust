//! The name service calls of `java.net` (`ast::Native::Net`): what the JVM's natives of
//! `InetAddress` and its implementations ask the C library, on the bytes of host names and
//! addresses, for the translator's `java.net.TranslatedNet`
//! (`java/overlay/java/net/TranslatedNet.java`), which makes the JDK's objects of them. None of
//! these opens a connection: they look names up (`getaddrinfo`) and addresses
//! (`getnameinfo`), name the host (`gethostname`),
//! list the network interfaces and their addresses (`getifaddrs` and `/proc/net/if_inet6`, as
//! the JVM reads them) and ask about one (`ioctl`), and test whether the system
//! makes sockets of an address family, closing the one it makes at once, as the JVM does when
//! it starts. A failed lookup records its `getaddrinfo` error, which `net_last_error` gives.

use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_void};

thread_local! {
    /// The `getaddrinfo` error of this thread's last lookup, 0 where it succeeded.
    static LAST_ERROR: Cell<i32> = const { Cell::new(0) };
    /// The `errno` of this thread's last question about a network interface
    /// (`net_interface_flags`, `net_hardware_address`, `net_mtu`), 0 where it succeeded.
    static INTERFACE_ERROR: Cell<i32> = const { Cell::new(0) };
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
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn if_nametoindex(name: *const c_char) -> u32;
    fn __errno_location() -> *mut i32;
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

const SIOCGIFFLAGS: u64 = 0x8913;
const SIOCGIFMTU: u64 = 0x8921;
const SIOCGIFHWADDR: u64 = 0x8927;
const SOCK_DGRAM: i32 = 2;
const IFF_BROADCAST: u32 = 0x2;

/// C's `struct ifreq`: an interface's name, and the union that each `ioctl` answers in.
#[repr(C)]
struct IfReq {
    name: [u8; 16],
    data: [u8; 24],
}

/// One network interface as the JVM's `enumInterfaces` lists it: its name and index, and its
/// addresses, each with its prefix length, an IPv4 address's broadcast address and an IPv6
/// address's scope (the interface's index).
struct Interface {
    name: Vec<u8>,
    index: i32,
    addresses: Vec<Address>,
}

/// An address of a network interface: its bytes, its prefix length, an IPv4 address's broadcast
/// address and an IPv6 address's scope.
struct Address {
    bytes: Vec<u8>,
    prefix: u8,
    broadcast: Option<Vec<u8>>,
    scope: u32,
}

/// The index of the interface named `name`, -1 where there is none.
fn index_of(name: &[u8]) -> i32 {
    let Ok(name) = CString::new(name) else { return -1 };
    // Safety: a C string.
    match unsafe { if_nametoindex(name.as_ptr()) } {
        0 => -1,
        index => index as i32,
    }
}

/// The JVM's `addif`: the address goes first on its interface's list, and an interface not yet
/// seen goes first on the list of interfaces.
fn add_address(list: &mut Vec<Interface>, name: &[u8], address: Address) {
    let at = match list.iter().position(|i| i.name == name) {
        Some(at) => at,
        None => {
            list.insert(0, Interface { name: name.to_vec(), index: index_of(name), addresses: Vec::new() });
            0
        }
    };
    list[at].addresses.insert(0, address);
}

/// The interfaces of the system with an address, as the JVM's `NetworkInterface.getAll()` lists
/// them on Linux: the IPv4 addresses (`getifaddrs`, in the kernel's order) and then the IPv6 ones
/// (`/proc/net/if_inet6`, in its order), each new interface and address put first. An IPv4
/// alias (`eth0:1`) is an interface of its own name, where the JVM makes it a virtual child of
/// its parent.
fn interfaces() -> Vec<Interface> {
    let mut list = Vec::new();
    let mut addrs: *mut IfAddrs = std::ptr::null_mut();
    // Safety: `addrs` receives the interfaces' addresses, freed below.
    if unsafe { getifaddrs(&mut addrs) } == 0 {
        let mut at = addrs;
        while !at.is_null() {
            // Safety: `at` is an entry of the list that `getifaddrs` gave; its addresses are
            // null or of the family they say.
            let entry = unsafe { &*at };
            at = entry.ifa_next;
            let is_v4 =
                !entry.ifa_addr.is_null() && i32::from(unsafe { (*entry.ifa_addr).sa_family }) == AF_INET;
            if !is_v4 {
                continue;
            }
            let Some((bytes, _)) = (unsafe { address_of(entry.ifa_addr) }) else { continue };
            let prefix = unsafe { address_of(entry.ifa_netmask) }
                .map_or(0, |(mask, _)| mask.iter().map(|b| b.count_ones()).sum::<u32>() as u8);
            let broadcast = if entry.ifa_flags & IFF_BROADCAST != 0 {
                unsafe { address_of(entry.ifa_ifu) }.map(|(b, _)| b)
            } else {
                None
            };
            // Safety: an entry's name is a C string.
            let name = unsafe { CStr::from_ptr(entry.ifa_name) }.to_bytes();
            add_address(&mut list, name, Address { bytes, prefix, broadcast, scope: 0 });
        }
        // Safety: the list that `getifaddrs` gave, not used after this.
        unsafe { freeifaddrs(addrs) };
    }
    for line in std::fs::read_to_string("/proc/net/if_inet6").unwrap_or_default().lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [hex, index, prefix, _scope, _flags, name] = fields[..] else { continue };
        let bytes: Option<Vec<u8>> =
            (0..16).map(|i| hex.get(2 * i..2 * i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect();
        let (Some(bytes), Ok(index), Ok(prefix)) =
            (bytes, u32::from_str_radix(index, 16), u8::from_str_radix(prefix, 16))
        else {
            continue;
        };
        add_address(&mut list, name.as_bytes(), Address { bytes, prefix, broadcast: None, scope: index });
    }
    list
}

/// `NetworkInterface.getAll()`'s interfaces, in its order, for `TranslatedNet` to make the
/// objects of: for each, the length and bytes of its name, its index (4 bytes, big-endian) and
/// the number of its addresses; for each address, its length (4 or 16) and bytes, its prefix
/// length, the length (0 or 4) and bytes of its broadcast address, and its scope (4 bytes).
pub fn net_interfaces() -> Vec<i8> {
    let mut out: Vec<u8> = Vec::new();
    for interface in interfaces() {
        out.push(interface.name.len() as u8);
        out.extend(&interface.name);
        out.extend(interface.index.to_be_bytes());
        out.push(interface.addresses.len() as u8);
        for address in interface.addresses {
            out.push(address.bytes.len() as u8);
            out.extend(&address.bytes);
            out.push(address.prefix);
            let broadcast = address.broadcast.unwrap_or_default();
            out.push(broadcast.len() as u8);
            out.extend(&broadcast);
            out.extend(address.scope.to_be_bytes());
        }
    }
    to_bytes(&out)
}

/// `ioctl(request)` of the interface named by `name`'s bytes on a datagram socket, as the JVM
/// asks: the union it answers in, or `None` with the `errno` recorded.
fn interface_ioctl(name: &[i8], request: u64) -> Option<[u8; 24]> {
    let mut req = IfReq { name: [0; 16], data: [0; 24] };
    for (to, &b) in req.name.iter_mut().zip(name.iter().take(15)) {
        *to = b as u8;
    }
    // Safety: plain calls on a descriptor of our own, closed below; `req` is the `ifreq` the
    // request reads and writes.
    let (done, error) = unsafe {
        let fd = socket(AF_INET, SOCK_DGRAM, 0);
        if fd < 0 {
            (false, *__errno_location())
        } else {
            let done = ioctl(fd, request, &mut req as *mut IfReq) >= 0;
            let error = if done { 0 } else { *__errno_location() };
            close(fd);
            (done, error)
        }
    };
    INTERFACE_ERROR.with(|e| e.set(error));
    done.then_some(req.data)
}

/// The `errno` of this thread's last question about an interface, 0 where it succeeded.
pub fn net_interface_error() -> i32 {
    INTERFACE_ERROR.with(Cell::get)
}

/// `SIOCGIFFLAGS` of the interface named `name`: its flags, or -1 with the error recorded.
pub fn net_interface_flags(name: &[i8]) -> i32 {
    interface_ioctl(name, SIOCGIFFLAGS).map_or(-1, |data| i32::from(u16::from_ne_bytes([data[0], data[1]])))
}

/// `SIOCGIFMTU` of the interface named `name`, or -1 with the error recorded.
pub fn net_mtu(name: &[i8]) -> i32 {
    interface_ioctl(name, SIOCGIFMTU)
        .map_or(-1, |data| i32::from_ne_bytes([data[0], data[1], data[2], data[3]]))
}

/// `SIOCGIFHWADDR` of the interface named `name`: its 6 bytes, none where they are all zero (no
/// hardware address, as the loopback's), or none with the error recorded where it fails.
pub fn net_hardware_address(name: &[i8]) -> Vec<i8> {
    match interface_ioctl(name, SIOCGIFHWADDR) {
        // After the address's family, two bytes.
        Some(data) if data[2..8].iter().any(|&b| b != 0) => to_bytes(&data[2..8]),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loopback_interface_is_listed_with_its_address_and_flags() {
        let lo: Vec<i8> = to_bytes(b"lo");
        let listed = interfaces();
        let loopback = listed.iter().find(|i| i.name == b"lo").expect("lo is listed");
        assert!(loopback.index > 0);
        assert!(loopback.addresses.iter().any(|a| a.bytes == [127, 0, 0, 1] && a.prefix == 8));
        assert!(!net_interfaces().is_empty());
        const IFF_LOOPBACK: i32 = 0x8;
        assert_ne!(net_interface_flags(&lo) & IFF_LOOPBACK, 0);
        assert!(net_mtu(&lo) > 0);
        assert!(net_hardware_address(&lo).is_empty());
        assert_eq!(net_interface_error(), 0);
        assert_eq!(net_interface_flags(&to_bytes(b"no-such-if")), -1);
        assert_eq!(net_interface_error(), 19);
    }
}
