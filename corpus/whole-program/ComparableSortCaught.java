import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

// The JDK's sorts of a mixed array and list: each compares with `compareTo` of a `Comparable`
// whose bridge method casts the other element, so a mixed sort fails with a ClassCastException
// that a try around it catches. The array keeps the elements the sort had not moved.
public class ComparableSortCaught {
    public static void main(String[] args) {
        Object[] mixed = {"a", 1};
        try {
            Arrays.sort(mixed);
        } catch (ClassCastException e) {
            System.out.println("sorted " + e.getMessage());
        }
        System.out.println(Arrays.toString(mixed));
        List<Object> items = new ArrayList<>(List.of("a", 1));
        try {
            Collections.sort((List) items);
        } catch (ClassCastException e) {
            System.out.println("listed " + e.getMessage());
        }
        Object[] same = {2, 1, 3};
        Arrays.sort(same);
        System.out.println(Arrays.toString(same));
    }
}
