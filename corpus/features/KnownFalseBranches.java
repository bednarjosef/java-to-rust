// A branch under a flag that is always false raises nothing that a `catch` could see: the flag
// is `Boolean.getBoolean` of a system property, which a translated program never has set, as
// the JDK's `Arrays.LegacyMergeSort.userRequested` is.
public class KnownFalseBranches {
    static class Legacy {
        static final boolean REQUESTED = Boolean.getBoolean("example.useLegacySplit");
    }

    static String[] parts(String s, String separator) {
        if (Legacy.REQUESTED) {
            return s.split(separator);
        }
        int at = s.indexOf(separator);
        return at < 0 ? new String[] {s} : new String[] {s.substring(0, at), s.substring(at + separator.length())};
    }

    public static void main(String[] args) {
        String separator = args.length > 0 ? args[0] : ",";
        try {
            String[] p = parts("left,right", separator);
            System.out.println(p.length + " " + p[0] + " " + p[p.length - 1]);
            System.out.println(parts("alone", separator).length);
        } catch (RuntimeException e) {
            System.out.println("failed");
        }
    }
}
