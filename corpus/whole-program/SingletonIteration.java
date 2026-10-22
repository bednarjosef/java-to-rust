import java.util.Collections;
import java.util.Iterator;
import java.util.Set;

public class SingletonIteration {
    public static void main(String[] args) {
        Set<String> one = Collections.singleton("only");
        Iterator<String> it = one.iterator();
        while (it.hasNext()) {
            System.out.println(it.next());
        }
        for (String s : one) {
            System.out.println(s + "!");
        }
    }
}
