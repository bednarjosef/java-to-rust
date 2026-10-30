// A pattern under a negated `&&` that is one operand of an `||` chain, as the JDK's
// `SSLAlgorithmConstraints` tests `mode == null || !(session instanceof ExtendedSSLSession s &&
// useTLS12PlusSpec(s.getProtocol()))` and reads `s` after an early return.
public class PatternInNegatedAndOperand {
    interface Session { String protocol(); }

    static class Extended implements Session {
        final String protocol;
        Extended(String protocol) { this.protocol = protocol; }
        public String protocol() { return protocol; }
        String algorithms() { return "ed25519,rsa_pss"; }
    }

    static class Plain implements Session {
        public String protocol() { return "TLSv1.3"; }
    }

    static class Constraints {
        boolean disabled;
        StringBuilder supported = new StringBuilder();

        Constraints(Session session, String mode) {
            if (mode == null
                    || !(session instanceof Extended ext
                    && ext.protocol().compareTo("TLSv1.2") >= 0)) {
                disabled = true;
                return;
            }
            supported.append(ext.algorithms());
            supported.append(" ").append(mode).append(":").append(ext.protocol());
        }
    }

    static String describe(Object o, boolean strict) {
        if (strict || !(o instanceof String s && !s.isEmpty())) {
            return "none";
        } else {
            return "text " + s.length();
        }
    }

    public static void main(String[] args) {
        Session[] sessions = { new Extended("TLSv1.3"), new Extended("TLSv1.1"), new Plain() };
        for (Session session : sessions) {
            for (String mode : new String[] { "peer", null }) {
                Constraints c = new Constraints(session, mode);
                System.out.println(c.disabled + " " + c.supported);
            }
        }
        System.out.println(describe("abc", false));
        System.out.println(describe("", false));
        System.out.println(describe("abc", true));
        System.out.println(describe(42, false));
    }
}
