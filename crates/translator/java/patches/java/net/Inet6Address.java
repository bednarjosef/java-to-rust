package java.net;

/**
 * The translator's patch of the JDK's {@code Inet6Address} (see docs/jdk-overlays.md): an
 * address of a network interface, which the JVM's natives make with the interface's index as
 * its scope and the interface itself as its scoped interface (so that it prints as
 * {@code fe80::1%eth0}), made by the translator's {@code NetworkInterface} through this factory.
 */
class Inet6Address {
    @Add
    static Inet6Address ofInterface(byte[] addr, int scopeId, NetworkInterface nif) {
        Inet6Address made = new Inet6Address(null, addr, scopeId);
        made.holder6.scope_ifname = nif;
        made.holder6.scope_ifname_set = true;
        return made;
    }
}
