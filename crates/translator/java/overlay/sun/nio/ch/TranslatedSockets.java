package sun.nio.ch;

import java.io.FileDescriptor;
import java.io.IOException;
import java.net.BindException;
import java.net.ConnectException;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.NoRouteToHostException;
import java.net.ProtocolException;
import java.net.SocketException;
import java.net.UnknownHostException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

import jdk.internal.access.JavaIOFileDescriptorAccess;
import jdk.internal.access.SharedSecrets;
import sun.net.ConnectionResetException;

/**
 * The translator's sockets, pipes and selectors of {@code sun.nio.ch}, in place of the JVM's
 * natives of {@code Net}, {@code IOUtil}, {@code SocketDispatcher}, {@code UnixDispatcher},
 * {@code EPoll}, {@code EventFD} and {@code NativeThread}, {@code closeIntFD} of
 * {@code UnixFileDispatcherImpl}, and {@code jdk.net.LinuxSocketOptions}'s: each call of one of those natives
 * calls the method here of the same parameters ({@code program::SOCKET_NATIVES}). The natives
 * here are the system's calls on a descriptor's number ({@code java_runtime::socket},
 * {@code ast::SOCKET_CALLS}); they record the error of a call that fails, and the methods throw
 * what the JVM's C code throws for it, with the system's text of the error as its message.
 * The answers about the platform are Linux's. Unix domain sockets are not supported, as on a
 * platform without them.
 */
final class TranslatedSockets {
    private TranslatedSockets() {}

    private static final JavaIOFileDescriptorAccess FD_ACCESS = SharedSecrets.getJavaIOFileDescriptorAccess();

    /** Linux's {@code errno} values that choose the exception, as the JVM's do. */
    private static final int EACCES = 13;
    private static final int EPIPE = 32;
    private static final int EPROTO = 71;
    private static final int EADDRINUSE = 98;
    private static final int EADDRNOTAVAIL = 99;
    private static final int ECONNABORTED = 103;
    private static final int ECONNRESET = 104;
    private static final int ENOTCONN = 107;
    private static final int ETIMEDOUT = 110;
    private static final int ECONNREFUSED = 111;
    private static final int EHOSTUNREACH = 113;

    // -- The system's calls (java_runtime::socket) --

    /** The {@code errno} of this thread's last call here, 0 where it succeeded. */
    private static native int sysLastError();

    /** {@code strerror(error)}. */
    private static native byte[] sysErrorText(int error);

    private static native boolean sysIPv6Available();

    private static native int sysOpen(boolean ipv6, boolean stream, boolean reuse);

    private static native int sysBind(int fd, boolean ipv6, byte[] address, int port);

    private static native int sysListen(int fd, int backlog);

    private static native int sysConnect(int fd, boolean ipv6, byte[] address, int port);

    private static native int sysAccept(int fd);

    /** The address and port of the peer of this thread's last {@code sysAccept}. */
    private static native byte[] sysAcceptedPeer();

    private static native byte[] sysLocalAddress(int fd);

    private static native byte[] sysRemoteAddress(int fd);

    private static native int sysGetOption(int fd, int level, int option);

    private static native int sysSetOption(int fd, int level, int option, int value);

    private static native int sysShutdown(int fd, int how);

    private static native int sysPoll(int fd, int events, long timeout);

    private static native int sysPollConnect(int fd, long timeout);

    private static native int sysAvailable(int fd);

    private static native int sysRead(int fd, long address, int len);

    private static native int sysWrite(int fd, long address, int len);

    private static native long sysReadv(int fd, long address, int len);

    private static native long sysWritev(int fd, long address, int len);

    private static native void sysClose(int fd);

    private static native void sysPreClose(int fd);

    private static native int sysConfigureBlocking(int fd, boolean blocking);

    private static native int sysFdLimit();

    private static native long sysMakePipe(boolean blocking);

    private static native int sysDrain(int fd);

    private static native int sysWriteByte(int fd, int b);

    private static native int sysEpollCreate();

    private static native int sysEpollControl(int epfd, int op, int fd, int events);

    private static native int sysEpollWait(int epfd, long address, int max, int timeout);

    private static native int sysEventFdOpen();

    private static native int sysEventFdSet(int fd);

    // -- Errors --

    /** The system's text of {@code error}. */
    private static String text(int error) {
        return new String(sysErrorText(error), StandardCharsets.UTF_8);
    }

    /** The exception of the JVM's {@code handleSocketError} for {@code error}. */
    private static IOException socketError(int error) {
        String message = text(error);
        return switch (error) {
            case EPROTO -> new ProtocolException(message);
            case ECONNREFUSED, ETIMEDOUT, ENOTCONN -> new ConnectException(message);
            case EHOSTUNREACH -> new NoRouteToHostException(message);
            case EADDRINUSE, EADDRNOTAVAIL, EACCES -> new BindException(message);
            default -> new SocketException(message);
        };
    }

    /** The JVM's {@code JNU_ThrowIOExceptionWithLastError}. */
    private static IOException ioError(int error) {
        return new IOException(text(error));
    }

    /** The JVM's {@code convertReturnVal}: the status a read or a write answers, or the throw. */
    private static int converted(int n) throws IOException {
        if (n == IOStatus.THROWN) {
            throw ioError(sysLastError());
        }
        return n;
    }

    /** A socket read's answer: a reset connection throws {@code ConnectionResetException}. */
    private static long readResult(long n) throws IOException {
        if (n == IOStatus.THROWN) {
            int error = sysLastError();
            if (error == ECONNRESET || error == EPIPE) {
                throw new ConnectionResetException("Connection reset");
            }
            throw ioError(error);
        }
        return n;
    }

    // -- Descriptors (IOUtil) --

    static int fdVal(FileDescriptor fd) {
        return FD_ACCESS.get(fd);
    }

    static void setfdVal(FileDescriptor fd, int value) {
        FD_ACCESS.set(fd, value);
    }

    /** {@code initIDs()} of {@code Net}, {@code IOUtil} and {@code UnixDispatcher}. */
    static void initIDs() {}

    static void configureBlocking(FileDescriptor fd, boolean blocking) throws IOException {
        if (sysConfigureBlocking(fdVal(fd), blocking) < 0) {
            throw ioError(sysLastError());
        }
    }

    static int fdLimit() {
        return sysFdLimit();
    }

    static long makePipe(boolean blocking) throws IOException {
        long fds = sysMakePipe(blocking);
        if (fds == -1) {
            throw ioError(sysLastError());
        }
        return fds;
    }

    static int write1(int fd, byte b) throws IOException {
        return converted(sysWriteByte(fd, b));
    }

    static boolean drain(int fd) throws IOException {
        int drained = sysDrain(fd);
        if (drained < 0) {
            throw ioError(sysLastError());
        }
        return drained == 1;
    }

    // -- Sockets (Net) --

    static boolean isIPv6Available0() {
        return sysIPv6Available();
    }

    static boolean isReusePortAvailable0() {
        return true;
    }

    /** Linux has no exclusive bind. */
    static int isExclusiveBindAvailable() {
        return -1;
    }

    static boolean shouldShutdownWriteBeforeClose0() {
        return false;
    }

    static boolean shouldSetBothIPv4AndIPv6Options0() {
        return true;
    }

    static boolean canIPv6SocketJoinIPv4Group0() {
        return true;
    }

    static boolean canJoin6WithIPv4Group0() {
        return true;
    }

    static boolean canUseIPv6OptionsWithIPv4LocalAddress0() {
        return true;
    }

    /** Linux's {@code poll} events. */
    static short pollinValue() {
        return 0x1;
    }

    static short polloutValue() {
        return 0x4;
    }

    static short pollerrValue() {
        return 0x8;
    }

    static short pollhupValue() {
        return 0x10;
    }

    static short pollnvalValue() {
        return 0x20;
    }

    static short pollconnValue() {
        return 0x4;
    }

    /** An IPv6 socket where the system has IPv6 and the caller prefers it, as the JVM makes. */
    static int socket0(boolean preferIPv6, boolean stream, boolean reuse, boolean fastLoopback)
            throws IOException {
        int fd = sysOpen(preferIPv6 && sysIPv6Available(), stream, reuse);
        if (fd < 0) {
            throw socketError(sysLastError());
        }
        return fd;
    }

    static void bind0(FileDescriptor fd, boolean preferIPv6, boolean useExclBind, InetAddress addr, int port)
            throws IOException {
        if (sysBind(fdVal(fd), preferIPv6, addr.getAddress(), port) < 0) {
            throw socketError(sysLastError());
        }
    }

    static void listen(FileDescriptor fd, int backlog) throws IOException {
        if (sysListen(fdVal(fd), backlog) < 0) {
            throw socketError(sysLastError());
        }
    }

    static int connect0(boolean preferIPv6, FileDescriptor fd, InetAddress remote, int remotePort)
            throws IOException {
        int n = sysConnect(fdVal(fd), preferIPv6, remote.getAddress(), remotePort);
        if (n == IOStatus.THROWN) {
            throw socketError(sysLastError());
        }
        return n;
    }

    /**
     * Accepts a connection into {@code newfd}, and its peer into {@code isaa[0]}: 1, or
     * {@code UNAVAILABLE} or {@code INTERRUPTED}. An aborted connection is skipped, as the JVM
     * does.
     */
    static int accept(FileDescriptor fd, FileDescriptor newfd, InetSocketAddress[] isaa) throws IOException {
        int accepted;
        do {
            accepted = sysAccept(fdVal(fd));
        } while (accepted == IOStatus.THROWN && sysLastError() == ECONNABORTED);
        if (accepted == IOStatus.UNAVAILABLE || accepted == IOStatus.INTERRUPTED) {
            return accepted;
        }
        if (accepted < 0) {
            throw ioError(sysLastError());
        }
        setfdVal(newfd, accepted);
        byte[] peer = sysAcceptedPeer();
        isaa[0] = new InetSocketAddress(address(peer), port(peer));
        return 1;
    }

    static void shutdown(FileDescriptor fd, int how) throws IOException {
        if (sysShutdown(fdVal(fd), how) < 0 && sysLastError() != ENOTCONN) {
            throw socketError(sysLastError());
        }
    }

    static int localPort(FileDescriptor fd) throws IOException {
        return port(named(sysLocalAddress(fdVal(fd))));
    }

    static InetAddress localInetAddress(FileDescriptor fd) throws IOException {
        return address(named(sysLocalAddress(fdVal(fd))));
    }

    static int remotePort(FileDescriptor fd) throws IOException {
        return port(named(sysRemoteAddress(fdVal(fd))));
    }

    static InetAddress remoteInetAddress(FileDescriptor fd) throws IOException {
        return address(named(sysRemoteAddress(fdVal(fd))));
    }

    /** The bytes of an address and its port that the system gave, or the throw where it failed. */
    private static byte[] named(byte[] bytes) throws IOException {
        if (bytes.length == 0) {
            throw socketError(sysLastError());
        }
        return bytes;
    }

    /** The address of bytes that end with a port, as the JVM's {@code NET_SockaddrToInetAddress}. */
    private static InetAddress address(byte[] bytes) {
        try {
            return InetAddress.getByAddress(Arrays.copyOf(bytes, bytes.length - 2));
        } catch (UnknownHostException e) {
            throw new InternalError(e);
        }
    }

    /** The port in the last two bytes, big-endian. */
    private static int port(byte[] bytes) {
        return ((bytes[bytes.length - 2] & 0xff) << 8) | (bytes[bytes.length - 1] & 0xff);
    }

    static int getIntOption0(FileDescriptor fd, boolean mayNeedConversion, int level, int opt)
            throws IOException {
        int value = sysGetOption(fdVal(fd), level, opt);
        if (value == -1 && sysLastError() != 0) {
            throw new SocketException(text(sysLastError()));
        }
        return value;
    }

    static void setIntOption0(FileDescriptor fd, boolean mayNeedConversion, int level, int opt, int arg,
            boolean isIPv6) throws IOException {
        if (sysSetOption(fdVal(fd), level, opt, arg) < 0) {
            throw new SocketException(text(sysLastError()));
        }
    }

    static int poll(FileDescriptor fd, int events, long timeout) throws IOException {
        int revents = sysPoll(fdVal(fd), events, timeout);
        if (revents < 0) {
            throw socketError(sysLastError());
        }
        return revents;
    }

    static boolean pollConnect(FileDescriptor fd, long timeout) throws IOException {
        int connected = sysPollConnect(fdVal(fd), timeout);
        if (connected < 0) {
            throw socketError(sysLastError());
        }
        return connected == 1;
    }

    static int available(FileDescriptor fd) throws IOException {
        int n = sysAvailable(fdVal(fd));
        if (n < 0) {
            throw socketError(sysLastError());
        }
        return n;
    }

    // -- Reads and writes (SocketDispatcher) and closing (UnixDispatcher) --

    static int read0(FileDescriptor fd, long address, int len) throws IOException {
        return (int) readResult(sysRead(fdVal(fd), address, len));
    }

    static long readv0(FileDescriptor fd, long address, int len) throws IOException {
        return readResult(sysReadv(fdVal(fd), address, len));
    }

    static int write0(FileDescriptor fd, long address, int len) throws IOException {
        return converted(sysWrite(fdVal(fd), address, len));
    }

    static long writev0(FileDescriptor fd, long address, int len) throws IOException {
        long n = sysWritev(fdVal(fd), address, len);
        if (n == IOStatus.THROWN) {
            throw ioError(sysLastError());
        }
        return n;
    }

    static void close0(FileDescriptor fd) throws IOException {
        int number = fdVal(fd);
        if (number != -1) {
            sysClose(number);
        }
    }

    /** {@code UnixFileDispatcherImpl.closeIntFD}, which a selector closes its descriptors with. */
    static void closeIntFD(int fd) throws IOException {
        sysClose(fd);
    }

    static void preClose0(FileDescriptor fd) throws IOException {
        sysPreClose(fdVal(fd));
    }

    /** Linux delivers a signal that a thread blocked in a call cannot take yet once it can. */
    static boolean supportPendingSignals0() {
        return true;
    }

    // -- Unix domain sockets (UnixDomainSockets), which the translation does not support --

    /** Not supported, so {@code SelectorProviderImpl} refuses the family before making one. */
    static boolean unixDomainInit() {
        return false;
    }

    private static UnsupportedOperationException unixDomain() {
        return new UnsupportedOperationException("Unix domain sockets are not supported in a translated program");
    }

    static int unixDomainSocket() throws IOException {
        throw unixDomain();
    }

    static void unixDomainBind(FileDescriptor fd, byte[] path) throws IOException {
        throw unixDomain();
    }

    static int unixDomainConnect(FileDescriptor fd, byte[] path) throws IOException {
        throw unixDomain();
    }

    static int unixDomainAccept(FileDescriptor fd, FileDescriptor newfd, Object[] array) throws IOException {
        throw unixDomain();
    }

    static byte[] unixDomainLocalAddress(FileDescriptor fd) throws IOException {
        throw unixDomain();
    }

    // -- Selectors (EPoll, EventFD) --

    /** The size of the kernel's {@code struct epoll_event}, packed on x86-64. */
    static int eventSize() {
        return 12;
    }

    static int eventsOffset() {
        return 0;
    }

    static int dataOffset() {
        return 4;
    }

    static int epollCreate() throws IOException {
        int epfd = sysEpollCreate();
        if (epfd < 0) {
            throw ioError(sysLastError());
        }
        return epfd;
    }

    /** 0, or the {@code errno} where it fails, as the JVM answers. */
    static int epollControl(int epfd, int opcode, int fd, int events) {
        return sysEpollControl(epfd, opcode, fd, events);
    }

    static int epollWait(int epfd, long pollAddress, int numfds, int timeout) throws IOException {
        int n = sysEpollWait(epfd, pollAddress, numfds, timeout);
        if (n == -1) {
            throw ioError(sysLastError());
        }
        return n;
    }

    static int eventfd0() throws IOException {
        int efd = sysEventFdOpen();
        if (efd < 0) {
            throw ioError(sysLastError());
        }
        return efd;
    }

    static int set0(int efd) throws IOException {
        if (sysEventFdSet(efd) < 0) {
            throw ioError(sysLastError());
        }
        return 0;
    }

    // -- jdk.net's socket options on Linux (LinuxSocketOptions), as libextnet's C code sets them --

    private static final int SOL_SOCKET = 1;
    private static final int IPPROTO_IP = 0;
    private static final int IPPROTO_TCP = 6;
    private static final int IPPROTO_IPV6 = 41;
    private static final int TCP_KEEPIDLE = 4;
    private static final int TCP_KEEPINTVL = 5;
    private static final int TCP_KEEPCNT = 6;
    private static final int TCP_QUICKACK = 12;
    private static final int SO_INCOMING_NAPI_ID = 56;
    private static final int IP_MTU_DISCOVER = 10;
    private static final int IPV6_MTU_DISCOVER = 23;
    private static final int IP_PMTUDISC_DONT = 0;
    private static final int IP_PMTUDISC_DO = 2;
    private static final int ENOPROTOOPT = 92;
    private static final int EPFNOSUPPORT = 96;
    private static final int EAFNOSUPPORT = 97;

    /**
     * {@code socketOptionSupported}: whether a new TCP socket, IPv6 where the system has it, can
     * read the option, which fails only for an option the system does not know.
     */
    private static boolean optionSupported(int level, int option) {
        int fd = sysOpen(true, true, false);
        if (fd < 0) {
            int error = sysLastError();
            if (error != EPFNOSUPPORT && error != EAFNOSUPPORT) {
                return false;
            }
            fd = sysOpen(false, true, false);
            if (fd < 0) {
                return false;
            }
        }
        boolean supported = sysGetOption(fd, level, option) != -1 || sysLastError() != ENOPROTOOPT;
        sysClose(fd);
        return supported;
    }

    static boolean keepAliveOptionsSupported0() {
        return optionSupported(IPPROTO_TCP, TCP_KEEPIDLE)
                && optionSupported(IPPROTO_TCP, TCP_KEEPCNT)
                && optionSupported(IPPROTO_TCP, TCP_KEEPINTVL);
    }

    /** As the C code, which sets the option on an IPv4 socket at the level of sockets. */
    static boolean quickAckSupported0() {
        int fd = sysOpen(false, true, false);
        if (fd < 0) {
            return false;
        }
        boolean supported = sysSetOption(fd, SOL_SOCKET, TCP_QUICKACK, 1) == 0 || sysLastError() != ENOPROTOOPT;
        sysClose(fd);
        return supported;
    }

    static boolean incomingNapiIdSupported0() {
        return optionSupported(SOL_SOCKET, SO_INCOMING_NAPI_ID);
    }

    /**
     * libextnet's {@code handleError}: an option the system does not know is unsupported, any
     * other failure a {@code SocketException} with the system's text of the error.
     */
    private static SocketException optionError(String what) {
        int error = sysLastError();
        if (error == ENOPROTOOPT) {
            throw new UnsupportedOperationException("unsupported socket option");
        }
        String message = text(error);
        return new SocketException(message.isEmpty() ? what : message);
    }

    private static void setTcpOption(int fd, int option, int value, String name) throws SocketException {
        if (sysSetOption(fd, IPPROTO_TCP, option, value) < 0) {
            throw optionError("set option " + name + " failed");
        }
    }

    private static int getOption(int fd, int level, int option, String name) throws SocketException {
        int value = sysGetOption(fd, level, option);
        if (value == -1 && sysLastError() != 0) {
            throw optionError("get option " + name + " failed");
        }
        return value;
    }

    static void setTcpKeepAliveProbes0(int fd, int value) throws SocketException {
        setTcpOption(fd, TCP_KEEPCNT, value, "TCP_KEEPCNT");
    }

    static void setTcpKeepAliveTime0(int fd, int value) throws SocketException {
        setTcpOption(fd, TCP_KEEPIDLE, value, "TCP_KEEPIDLE");
    }

    static void setTcpKeepAliveIntvl0(int fd, int value) throws SocketException {
        setTcpOption(fd, TCP_KEEPINTVL, value, "TCP_KEEPINTVL");
    }

    static int getTcpKeepAliveProbes0(int fd) throws SocketException {
        return getOption(fd, IPPROTO_TCP, TCP_KEEPCNT, "TCP_KEEPCNT");
    }

    static int getTcpKeepAliveTime0(int fd) throws SocketException {
        return getOption(fd, IPPROTO_TCP, TCP_KEEPIDLE, "TCP_KEEPIDLE");
    }

    static int getTcpKeepAliveIntvl0(int fd) throws SocketException {
        return getOption(fd, IPPROTO_TCP, TCP_KEEPINTVL, "TCP_KEEPINTVL");
    }

    static void setQuickAck0(int fd, boolean on) throws SocketException {
        setTcpOption(fd, TCP_QUICKACK, on ? 1 : 0, "TCP_QUICKACK");
    }

    static boolean getQuickAck0(int fd) throws SocketException {
        return getOption(fd, IPPROTO_TCP, TCP_QUICKACK, "TCP_QUICKACK") != 0;
    }

    static void setIpDontFragment0(int fd, boolean value, boolean isIPv6) throws SocketException {
        int level = isIPv6 ? IPPROTO_IPV6 : IPPROTO_IP;
        int option = isIPv6 ? IPV6_MTU_DISCOVER : IP_MTU_DISCOVER;
        if (sysSetOption(fd, level, option, value ? IP_PMTUDISC_DO : IP_PMTUDISC_DONT) < 0) {
            throw optionError("set option IP_DONTFRAGMENT failed");
        }
    }

    static boolean getIpDontFragment0(int fd, boolean isIPv6) throws SocketException {
        int level = isIPv6 ? IPPROTO_IPV6 : IPPROTO_IP;
        int option = isIPv6 ? IPV6_MTU_DISCOVER : IP_MTU_DISCOVER;
        return getOption(fd, level, option, "IP_DONTFRAGMENT") == IP_PMTUDISC_DO;
    }

    static int getIncomingNapiId0(int fd) throws SocketException {
        return getOption(fd, SOL_SOCKET, SO_INCOMING_NAPI_ID, "SO_INCOMING_NAPI_ID");
    }

    /**
     * The peer's user and group of a Unix domain socket, which only those sockets have
     * ({@code ExtendedSocketOptions} offers {@code SO_PEERCRED} on no other). A translated program
     * has none, so what is asked is an internet socket, whose peer Linux gives no user: the JDK's
     * C code then throws this.
     */
    static long getSoPeerCred0(int fd) throws SocketException {
        throw new SocketException("get peer uid failed");
    }
}
