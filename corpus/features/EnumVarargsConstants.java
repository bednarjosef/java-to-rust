import java.util.Arrays;

// An enum whose constructor takes a variable number of arguments, as the JDK's
// `X509Authentication` (`RSA_OR_PSS("RSA_OR_PSS", "RSA", "RSASSA-PSS")`): each constant's trailing
// arguments are one array, made once, so reads of the field see the same array and its changes.
public class EnumVarargsConstants {
    enum Auth {
        NONE("none"),
        RSA("RSA", "RSA"),
        RSA_OR_PSS("RSA_OR_PSS", "RSA", "RSASSA-PSS");

        final String keyAlgorithm;
        final String[] keyTypes;

        Auth(String keyAlgorithm, String... keyTypes) {
            this.keyAlgorithm = keyAlgorithm;
            this.keyTypes = keyTypes;
        }
    }

    public static void main(String[] args) {
        for (Auth a : Auth.values()) {
            System.out.println(a + " " + a.keyAlgorithm + " " + a.keyTypes.length + " " + Arrays.toString(a.keyTypes));
        }
        Auth pick = args.length > 0 ? Auth.NONE : Auth.RSA_OR_PSS;
        System.out.println(pick.keyTypes == pick.keyTypes);
        pick.keyTypes[0] = "changed";
        System.out.println(Arrays.toString(Auth.RSA_OR_PSS.keyTypes));
    }
}
