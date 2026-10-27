package sun.net.spi;

import java.net.Proxy;

/**
 * The translator's patch of the JDK's {@code DefaultProxySelector} (see docs/jdk-overlays.md):
 * the system's proxy settings, which the JVM reads from GNOME's libraries where
 * {@code java.net.useSystemProxies} is true, are never found, as on a system without those
 * libraries. So only the {@code http.proxyHost} and other proxy properties choose a proxy.
 */
class DefaultProxySelector {
    @Replace
    private static boolean init() {
        return false;
    }

    @Replace
    private synchronized Proxy[] getSystemProxies(String protocol, String host) {
        return null;
    }
}
