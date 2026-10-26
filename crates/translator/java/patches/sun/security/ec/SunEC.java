package sun.security.ec;

import sun.security.util.KnownOIDs;

/**
 * The translator's patch of the JDK's {@code SunEC} (see docs/jdk-overlays.md): its services make
 * their implementations by name, as the other providers' do ({@code Provider.Service}), rather
 * than each with its constructor, as the elliptic curve classes are not translated; and the
 * curves that its EC parameters support are named from their object identifiers, not from
 * {@code CurveDB}'s curves, whose parameters and encodings are not needed to register them.
 */
class SunEC {
    @Edit(from = "for (NamedCurve namedCurve", to = "HashMap<String, String> apAttrs", replace = """
            for (KnownOIDs curve : List.of(
                    KnownOIDs.secp256r1, KnownOIDs.secp384r1, KnownOIDs.secp521r1)) {
                if (!firstCurve) {
                    names.append("|");
                } else {
                    firstCurve = false;
                }
                names.append("[").append(curve.stdName()).append(",");
                for (String alias : curve.aliases()) {
                    names.append(alias).append(",");
                }
                names.append(curve.value()).append("]");
            }

            HashMap<String, String> apAttrs""")
    void putEntries();

    @Patch
    private static class ProviderService {
        @Replace
        @Override
        public Object newInstance(Object ctrParamObj) throws NoSuchAlgorithmException {
            return super.newInstance(ctrParamObj);
        }
    }
}
