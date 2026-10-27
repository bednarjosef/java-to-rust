package java.net;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import jdk.internal.util.Exceptions;

/**
 * The translator's name service of {@code java.net}, in place of the JVM's natives of
 * {@code Inet4AddressImpl}, {@code Inet6AddressImpl} and {@code NetworkInterface}: the natives
 * here are the C library's calls on the bytes of a name or an address ({@code java_runtime::net},
 * {@code ast::NET_CALLS}), and the methods make the JDK's objects of what they give, as the
 * JVM's natives do. Nothing here opens a connection. The interfaces that the JVM's natives of
 * {@code NetworkInterface} list and ask about come from here too, through its patch.
 */
final class TranslatedNet {
    private TranslatedNet() {}

    /** The bits of {@code InetAddressResolver.LookupPolicy.characteristics()}. */
    private static final int IPV4 = 1 << 0;
    private static final int IPV6 = 1 << 1;
    private static final int IPV4_FIRST = 1 << 2;
    private static final int IPV6_FIRST = 1 << 3;

    /**
     * {@code getaddrinfo} of {@code host} for {@code family} (4, 6, or 0 for both): each address
     * once, as its length, its bytes and an IPv6 address's scope; empty with the error recorded
     * where it fails.
     */
    static native byte[] lookup0(byte[] host, int family);

    /** The {@code getaddrinfo} error of this thread's last {@code lookup0}, 0 where it succeeded. */
    static native int lastError0();

    /** {@code gai_strerror(code)}. */
    static native byte[] errorText0(int code);

    /**
     * {@code getnameinfo} of an address's bytes: the name the system has for it, or empty with
     * the error recorded where it has none.
     */
    static native byte[] hostOf0(byte[] address);

    /** {@code gethostname()}, or {@code localhost} where it fails. */
    static native byte[] hostname0();

    /** Whether a network interface has the address of these bytes. */
    static native boolean bound0(byte[] address);

    /**
     * The network interfaces and their addresses, in the order of the JVM's
     * {@code NetworkInterface.getAll()}, as {@code java_runtime::net_interfaces} encodes them;
     * {@code NetworkInterface}'s patch makes its objects of them.
     */
    static native byte[] interfaces0();

    /** {@code SIOCGIFFLAGS} of the interface of this name, or -1 with the error recorded. */
    static native int interfaceFlags0(byte[] name);

    /** {@code SIOCGIFMTU} of the interface of this name, or -1 with the error recorded. */
    static native int interfaceMtu0(byte[] name);

    /**
     * {@code SIOCGIFHWADDR} of the interface of this name: its bytes, or none where they are all
     * zero or with the error recorded where it fails.
     */
    static native byte[] hardwareAddress0(byte[] name);

    /** The {@code errno} of this thread's last question about an interface, 0 where it succeeded. */
    static native int interfaceError0();

    /** {@code strerror(error)}. */
    static native byte[] systemErrorText0(int error);

    /** The system's text of {@code error}, as the JVM's exceptions about an interface give it. */
    static String systemErrorText(int error) {
        return new String(systemErrorText0(error), StandardCharsets.UTF_8);
    }

    /** {@code getLocalHostName()} of {@code Inet4AddressImpl} and {@code Inet6AddressImpl}. */
    static String localHostName() {
        return new String(hostname0(), StandardCharsets.UTF_8);
    }

    /**
     * The JVM's {@code lookupAllHostAddr} of {@code Inet4AddressImpl} ({@code IPV4}) and
     * {@code Inet6AddressImpl}: the addresses of {@code host} of the families that
     * {@code characteristics} names, with the IPv4 or the IPv6 ones first where it says so, each
     * named {@code host}. A name that is not found throws {@code UnknownHostException} with the C
     * library's text, after the name where exceptions show it.
     */
    static InetAddress[] lookup(String host, int characteristics) throws UnknownHostException {
        int families = characteristics & (IPV4 | IPV6);
        int family = families == IPV4 ? 4 : families == IPV6 ? 6 : 0;
        byte[] found = lookup0(host.getBytes(StandardCharsets.UTF_8), family);
        int error = lastError0();
        if (error != 0) {
            String text = new String(errorText0(error), StandardCharsets.UTF_8);
            throw new UnknownHostException(Exceptions.enhancedNonSocketExceptions() ? host + ": " + text : text);
        }
        List<InetAddress> all = new ArrayList<>();
        List<InetAddress> v4 = new ArrayList<>();
        List<InetAddress> v6 = new ArrayList<>();
        int at = 0;
        while (at < found.length) {
            int length = found[at++];
            byte[] bytes = Arrays.copyOfRange(found, at, at + length);
            at += length;
            if (length == 4) {
                Inet4Address address = new Inet4Address();
                address.holder().address = ((bytes[0] & 0xff) << 24) | ((bytes[1] & 0xff) << 16)
                        | ((bytes[2] & 0xff) << 8) | (bytes[3] & 0xff);
                address.holder().hostName = host;
                all.add(address);
                v4.add(address);
            } else {
                int scope = ((found[at] & 0xff) << 24) | ((found[at + 1] & 0xff) << 16)
                        | ((found[at + 2] & 0xff) << 8) | (found[at + 3] & 0xff);
                at += 4;
                Inet6Address address = new Inet6Address(host, bytes, scope != 0 ? scope : -1);
                // The JVM names the address it makes, which it has no original name for.
                address.holder().originalHostName = null;
                all.add(address);
                v6.add(address);
            }
        }
        List<InetAddress> ordered = all;
        if ((characteristics & IPV6_FIRST) != 0) {
            ordered = new ArrayList<>(v6);
            ordered.addAll(v4);
        } else if ((characteristics & IPV4_FIRST) != 0) {
            ordered = new ArrayList<>(v4);
            ordered.addAll(v6);
        }
        return ordered.toArray(new InetAddress[0]);
    }

    /**
     * The JVM's {@code getHostByAddr} of {@code Inet4AddressImpl} and {@code Inet6AddressImpl}:
     * the name of the address of these bytes, or {@code UnknownHostException} without a message
     * where the system has none.
     */
    static String hostOf(byte[] address) throws UnknownHostException {
        byte[] name = hostOf0(address);
        if (lastError0() != 0) {
            throw new UnknownHostException();
        }
        return new String(name, StandardCharsets.UTF_8);
    }

    /**
     * {@code NetworkInterface.boundInetAddress0(addr)}: whether an interface has the address. A
     * null address fails without a message, as in the JVM's C code.
     */
    static boolean boundInetAddress0(InetAddress addr) throws SocketException {
        if (addr == null) {
            throw new NullPointerException();
        }
        return bound0(addr.getAddress());
    }
}
