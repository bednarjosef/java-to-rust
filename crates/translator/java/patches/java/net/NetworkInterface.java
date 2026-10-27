package java.net;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * The translator's patch of the JDK's {@code NetworkInterface} (see docs/jdk-overlays.md): the
 * natives that list the interfaces and ask about one are the translator's
 * ({@code TranslatedNet}, {@code java_runtime::net}) in place of the JVM's. The interfaces are
 * listed as the JVM lists them on Linux, each call making new objects: those with an address,
 * the IPv4 addresses first and then the IPv6 ones, each new interface and address put before
 * the earlier ones. An IPv4 alias ({@code eth0:1}) is an interface of its own, where the JVM
 * makes it a virtual child of its parent.
 */
class NetworkInterface {
    @Add
    private static final int IFF_UP = 0x1;

    @Add
    private static final int IFF_LOOPBACK = 0x8;

    @Add
    private static final int IFF_POINTOPOINT = 0x10;

    @Add
    private static final int IFF_RUNNING = 0x40;

    @Add
    private static final int IFF_MULTICAST = 0x1000;

    @Replace
    private static NetworkInterface[] getAll() throws SocketException {
        byte[] listed = TranslatedNet.interfaces0();
        List<NetworkInterface> all = new ArrayList<>();
        int at = 0;
        while (at < listed.length) {
            int nameLength = listed[at++] & 0xff;
            String name = new String(listed, at, nameLength, StandardCharsets.UTF_8);
            at += nameLength;
            int index = bigEndianInt(listed, at);
            at += 4;
            int count = listed[at++] & 0xff;
            NetworkInterface nif = new NetworkInterface(name, index, new InetAddress[count]);
            nif.displayName = name;
            nif.bindings = new InterfaceAddress[count];
            nif.childs = new NetworkInterface[0];
            for (int i = 0; i < count; i++) {
                int length = listed[at++] & 0xff;
                byte[] bytes = Arrays.copyOfRange(listed, at, at + length);
                at += length;
                short prefix = (short) (listed[at++] & 0xff);
                int broadcastLength = listed[at++] & 0xff;
                Inet4Address broadcast = broadcastLength == 4
                        ? new Inet4Address(null, Arrays.copyOfRange(listed, at, at + 4))
                        : null;
                at += broadcastLength;
                int scope = bigEndianInt(listed, at);
                at += 4;
                InetAddress address = length == 4
                        ? new Inet4Address(null, bytes)
                        : Inet6Address.ofInterface(bytes, scope, nif);
                nif.addrs[i] = address;
                nif.bindings[i] = InterfaceAddress.of(address, broadcast, prefix);
            }
            all.add(nif);
        }
        return all.toArray(new NetworkInterface[0]);
    }

    @Add
    private static int bigEndianInt(byte[] bytes, int at) {
        return ((bytes[at] & 0xff) << 24) | ((bytes[at + 1] & 0xff) << 16) | ((bytes[at + 2] & 0xff) << 8)
                | (bytes[at + 3] & 0xff);
    }

    @Replace
    private static NetworkInterface getByName0(String name) throws SocketException {
        for (NetworkInterface nif : getAll()) {
            if (nif.name.equals(name)) {
                return nif;
            }
        }
        return null;
    }

    @Replace
    private static NetworkInterface getByIndex0(int index) throws SocketException {
        for (NetworkInterface nif : getAll()) {
            if (nif.index == index) {
                return nif;
            }
        }
        return null;
    }

    @Replace
    private static NetworkInterface getByInetAddress0(InetAddress addr) throws SocketException {
        byte[] wanted = addr.getAddress();
        for (NetworkInterface nif : getAll()) {
            for (InetAddress bound : nif.addrs) {
                if (Arrays.equals(bound.getAddress(), wanted)) {
                    return nif;
                }
            }
        }
        return null;
    }

    /** {@code SIOCGIFFLAGS} of the interface, or the {@code SocketException} of its error. */
    @Add
    private static int flags(String name) throws SocketException {
        int flags = TranslatedNet.interfaceFlags0(name.getBytes(StandardCharsets.UTF_8));
        if (flags == -1) {
            throw new SocketException(TranslatedNet.systemErrorText(TranslatedNet.interfaceError0()));
        }
        return flags;
    }

    @Replace
    private static boolean isUp0(String name, int ind) throws SocketException {
        int flags = flags(name);
        return (flags & IFF_UP) != 0 && (flags & IFF_RUNNING) != 0;
    }

    @Replace
    private static boolean isLoopback0(String name, int ind) throws SocketException {
        return (flags(name) & IFF_LOOPBACK) != 0;
    }

    @Replace
    private static boolean supportsMulticast0(String name, int ind) throws SocketException {
        return (flags(name) & IFF_MULTICAST) != 0;
    }

    @Replace
    private static boolean isP2P0(String name, int ind) throws SocketException {
        return (flags(name) & IFF_POINTOPOINT) != 0;
    }

    @Replace
    private static byte[] getMacAddr0(byte[] inAddr, String name, int ind) throws SocketException {
        byte[] address = TranslatedNet.hardwareAddress0(name.getBytes(StandardCharsets.UTF_8));
        if (address.length > 0) {
            return address;
        }
        int error = TranslatedNet.interfaceError0();
        if (error != 0) {
            throw new SocketException(TranslatedNet.systemErrorText(error));
        }
        return null;
    }

    @Replace
    private static int getMTU0(String name, int ind) throws SocketException {
        int mtu = TranslatedNet.interfaceMtu0(name.getBytes(StandardCharsets.UTF_8));
        if (mtu == -1) {
            throw new SocketException(TranslatedNet.systemErrorText(TranslatedNet.interfaceError0()));
        }
        return mtu;
    }

    @Replace
    private static void init() {}
}
