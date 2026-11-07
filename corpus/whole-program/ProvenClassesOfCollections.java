import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.Iterator;
import java.util.LinkedList;
import java.util.List;
import java.util.Map;

// Casts and calls on the JDK's collections where an earlier test proved the class, and casts
// that are the receivers of calls the points-to facts narrow to a few classes.
public class ProvenClassesOfCollections {
    static int sum(Map<Long, Object> byKey) {
        int sum = 0;
        for (Map.Entry<Long, Object> entry : byKey.entrySet()) {
            // A cast that is the receiver of a call on one or two classes.
            Iterator<?> it = ((List<?>) entry.getValue()).iterator();
            while (it.hasNext()) {
                Object o = it.next();
                if (o instanceof Integer) {
                    sum += (Integer) o;
                }
            }
            Object value = entry.getValue();
            if (value instanceof ArrayList) {
                sum += ((ArrayList<?>) value).size() + ((List<?>) value).size() + ((Collection<?>) value).size();
            } else if (value instanceof List) {
                sum += 100 * ((List<?>) value).size();
            }
            if (!(value instanceof Collection)) continue;
            sum += ((Collection<?>) value).isEmpty() ? 1 : 2;
        }
        return sum;
    }

    public static void main(String[] args) {
        Map<Long, Object> byKey = new HashMap<>();
        for (long k = 0; k < 6; k++) {
            List<Object> list = k % 3 == 0 ? new LinkedList<>() : new ArrayList<>();
            for (int i = 0; i < k; i++) {
                list.add(i % 2 == 0 ? (Object) i : "s" + i);
            }
            byKey.put(k, list);
        }
        System.out.println(sum(byKey));
        try {
            byKey.put(9L, "not a list");
            System.out.println(sum(byKey));
        } catch (ClassCastException e) {
            System.out.println("not a list");
        }
    }
}
