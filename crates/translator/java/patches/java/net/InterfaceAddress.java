package java.net;

/**
 * The translator's patch of the JDK's {@code InterfaceAddress} (see docs/jdk-overlays.md): the
 * JVM's natives make its objects and set its private fields; the translator's
 * {@code NetworkInterface} makes them through this factory instead.
 */
class InterfaceAddress {
    @Add
    static InterfaceAddress of(InetAddress address, Inet4Address broadcast, short maskLength) {
        InterfaceAddress made = new InterfaceAddress();
        made.address = address;
        made.broadcast = broadcast;
        made.maskLength = maskLength;
        return made;
    }
}
