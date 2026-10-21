// Comparator.reverseOrder() and reversed(): Collections.ReverseComparator, whose class reaches
// AbstractList.listIterator and so the JDK's inner class ListItr, which extends the inner class
// Itr of the generic AbstractList; and ReverseComparator2, whose equals calls Comparator.equals,
// which restates Object's.
import java.util.Comparator;

public class ReverseOrder {
    static int sign(int x) {
        return Integer.signum(x);
    }

    public static void main(String[] args) {
        Comparator<String> reverse = Comparator.reverseOrder();
        System.out.println(sign(reverse.compare("a", "b")) + " " + sign(reverse.compare("b", "a")));
        Comparator<Integer> natural = Comparator.naturalOrder();
        Comparator<Integer> down = natural.reversed();
        System.out.println(sign(down.compare(1, 2)) + " " + sign(down.compare(2, 2)));
        Comparator<String> byLength = Comparator.comparingInt(String::length);
        System.out.println(sign(byLength.reversed().compare("aa", "b")));
    }
}
