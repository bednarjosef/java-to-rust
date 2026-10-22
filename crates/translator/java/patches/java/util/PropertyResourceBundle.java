package java.util;

import sun.util.TranslatedPropertyFiles;

/**
 * The translator's patch of the JDK's {@code PropertyResourceBundle} (see docs/jdk-overlays.md):
 * the constructor from a stream reads through the translator's {@code TranslatedPropertyFiles},
 * which decodes as the JDK's {@code PropertyResourceBundleCharset} does, and the property that
 * chose the JDK's decoding is not read.
 */
class PropertyResourceBundle {
    @Replace
    public PropertyResourceBundle (InputStream stream) throws IOException {
        this(TranslatedPropertyFiles.reader(stream));
    }

    @Remove
    private static final String encoding;
}
