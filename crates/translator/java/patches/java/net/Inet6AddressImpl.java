package java.net;

/**
 * The translator's patch of the JDK's {@code Inet6AddressImpl} (see docs/jdk-overlays.md): the
 * natives that name a host and look names up are the translator's name service
 * ({@code TranslatedNet}) in place of the JVM's. Its other native, {@code isReachable0} (which
 * sends packets), stays a native the translation does not have, rejected where it can run.
 */
class Inet6AddressImpl {
    @Replace
    public String getLocalHostName() throws UnknownHostException {
        return TranslatedNet.localHostName();
    }

    @Replace
    private InetAddress[] lookupAllHostAddr(String hostname, int characteristics)
            throws UnknownHostException {
        return TranslatedNet.lookup(hostname, characteristics);
    }

    @Replace
    public String getHostByAddr(byte[] addr) throws UnknownHostException {
        return TranslatedNet.hostOf(addr);
    }
}
