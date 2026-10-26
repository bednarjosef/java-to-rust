package sun.security.provider;

import java.net.URI;
import java.net.URISyntaxException;
import jdk.internal.util.StaticProperty;

/**
 * The translator's patch of the JDK's {@code NativePRNG} (see docs/jdk-overlays.md). The seed
 * source that {@code securerandom.source} or {@code java.security.egd} names, which must be a
 * {@code file:} URL, is read as a URI, not as a {@code URL}: the {@code SUN} provider asks
 * whether the generator is available as it registers its services, and a {@code URL} would bring
 * the protocol handlers into every program that looks a security service up.
 */
class NativePRNG {
    @Remove
    private static URL getEgdUrl();

    /** The seed source as {@code getEgdUrl} gives it, a URI rather than a URL. */
    @Add
    private static URI egdUri() {
        String egdSource = SunEntries.getSeedSource();
        if (egdSource.length() == 0) {
            return null;
        }
        try {
            URI uri = new URI(egdSource);
            return "file".equalsIgnoreCase(uri.getScheme()) ? uri : null;
        } catch (URISyntaxException e) {
            return null;
        }
    }

    /** The file of a {@code file:} URI, as {@code SunEntries.getDeviceFile} finds it. */
    @Add
    private static File deviceFile(URI device) {
        if (device.isOpaque()) {
            URI localDir = new File(StaticProperty.userDir()).toURI();
            return new File(URI.create(localDir.toString() + device.toString().substring(5)));
        }
        return new File(device);
    }

    @Edit(from = "URL egdUrl;", to = "// Swallow, seedFile is still null", replace = """
            File egdFile = null;
            URI egdUri;

            if ((egdUri = egdUri()) != null) {
                try {
                    egdFile = deviceFile(egdUri);
                } catch (IllegalArgumentException e) {
                    // Swallow, seedFile is still null""")
    private static RandomIO initIO(final Variant v);
}
