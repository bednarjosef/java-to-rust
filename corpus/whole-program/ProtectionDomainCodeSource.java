// A call on a class's protection domain, which the translation does not have: the code that
// asks for it builds, and ends the program where it runs (not here, without arguments).
import java.security.CodeSource;
public class ProtectionDomainCodeSource {
    public static void main(String[] args) {
        if (args.length > 0) {
            CodeSource source = ProtectionDomainCodeSource.class.getProtectionDomain().getCodeSource();
            System.out.println(source == null);
        }
        System.out.println("done");
    }
}
