package java.net;

/**
 * The translator's patch of the JDK's {@code InetAddress} (see docs/jdk-overlays.md). The built-in
 * resolver refuses a hosts file that the {@code jdk.net.hosts.file} property names with an
 * {@code UnsupportedOperationException}: the JDK's {@code HostsFileResolver} reads it through a
 * file channel over native memory, which a translated program does not have. Without the
 * property, names are looked up by the system, as the JDK's {@code PlatformResolver} does (through
 * {@code TranslatedNet}, see the patches of {@code Inet4AddressImpl} and {@code Inet6AddressImpl}).
 * A resolver of the program's own ({@code InetAddressResolverProvider}) is refused too, with an
 * {@code UnsupportedOperationException}: with the built-in resolver only, which throws nothing but
 * {@code UnknownHostException}, a lookup by name catches only that, where the JDK's also catches
 * the {@code RuntimeException} of a resolver of the program's own and makes it the cause of one.
 */
class InetAddress {
    // The built-in resolver only (see the class's comment).
    @Edit(from = "return ServiceLoader.load(", to = ".orElse(BUILTIN_RESOLVER);", replace = """
            // The built-in resolver only (see the class's comment).
            if (ServiceLoader.load(InetAddressResolverProvider.class).findFirst().isPresent()) {
                throw new UnsupportedOperationException("a resolver of the program's own, an InetAddressResolverProvider");
            }
            return BUILTIN_RESOLVER;
            """)
    private static InetAddressResolver loadResolver();

    @Patch
    private static final class PlatformResolver {
        // The JDK's assigns a local in the `try` and streams it after; the translation does not
        // yet declare an array local without a value there.
        @Edit(find = "InetAddress[] addrs;", replace = "")
        @Edit(find = "addrs = impl.lookupAllHostAddr(host, policy);",
                replace = "return Arrays.stream(impl.lookupAllHostAddr(host, policy));")
        @Edit(find = "return Arrays.stream(addrs);", replace = "")
        public Stream<InetAddress> lookupByName(String host, LookupPolicy policy);
    }

    // Its `HostsFileResolver` reads the file with a `Scanner` over a file channel, whose buffers
    // are native memory, which a translated program does not have.
    @Edit(find = "theResolver = new HostsFileResolver(HOSTS_FILE_NAME);", replace = """
            throw new UnsupportedOperationException("the hosts file that jdk.net.hosts.file names, "
                    + HOSTS_FILE_NAME);
            """)
    private static InetAddressResolver createBuiltinInetAddressResolver();

    // The built-in resolver throws no other exception than `UnknownHostException` (see the
    // class's comment).
    @Edit(find = "catch (RuntimeException | UnknownHostException x)", replace = "catch (UnknownHostException x)")
    @Edit(from = "} else if (x instanceof UnknownHostException uhe) {", to = "ex.initCause(x);", replace = """
            } else {
                ex = x;
            """)
    static InetAddress[] getAddressesFromNameService(String host);
}
