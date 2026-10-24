// `System.mapLibraryName`: the name of a native library's file on this system, a name too long
// for the JDK's buffer, and null.
public class LibraryNames {
    public static void main(String[] args) {
        System.out.println(System.mapLibraryName("zip"));
        System.out.println(System.mapLibraryName(""));
        System.out.println(System.mapLibraryName("x".repeat(240)).length());
        try {
            System.mapLibraryName("x".repeat(241));
        } catch (IllegalArgumentException e) {
            System.out.println(e);
        }
        try {
            System.mapLibraryName(null);
        } catch (NullPointerException e) {
            System.out.println(e);
        }
    }
}
