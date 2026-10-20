public class ComparableImpl {
    interface Labeled {
        String label();
    }

    static class Version implements Comparable<Version>, Labeled {
        final int major, minor;
        Version(int major, int minor) { this.major = major; this.minor = minor; }
        @Override
        public int compareTo(Version o) {
            return major != o.major ? Integer.compare(major, o.major) : Integer.compare(minor, o.minor);
        }
        public String label() { return major + "." + minor; }
    }

    static class Tag implements Labeled {
        final String t;
        Tag(String t) { this.t = t; }
        public String label() { return "#" + t; }
    }

    static void sort(Version[] vs) {
        for (int i = 1; i < vs.length; i++) {
            Version v = vs[i];
            int j = i - 1;
            while (j >= 0 && vs[j].compareTo(v) > 0) {
                vs[j + 1] = vs[j];
                j--;
            }
            vs[j + 1] = v;
        }
    }

    public static void main(String[] args) {
        Version[] vs = { new Version(1, 10), new Version(1, 2), new Version(0, 9), new Version(2, 0), new Version(1, 2) };
        sort(vs);
        StringBuilder sb = new StringBuilder();
        for (Version v : vs) sb.append(v.label()).append(' ');
        System.out.println(sb.toString().trim());
        System.out.println(vs[1].compareTo(vs[2]) + " " + vs[0].compareTo(vs[4]) + " " + vs[4].compareTo(vs[0]));
        Labeled[] all = { vs[0], new Tag("x"), vs[4] };
        for (Labeled l : all) System.out.print(l.label() + ";");
        System.out.println();
        System.out.println("b".compareTo("a") + " " + Integer.valueOf(3).compareTo(7));
    }
}
