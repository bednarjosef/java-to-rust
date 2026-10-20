// Features: ArrayList and LinkedList, List.of, add/remove/set/indexOf/subList, Iterator with
// remove, ListIterator, Collections.sort/reverse/max/min/frequency/swap/unmodifiableList,
// UnsupportedOperationException from an immutable list, toArray-free for-each loops.
import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.LinkedList;
import java.util.List;
import java.util.ListIterator;

public class ListOperations {
    static void show(String label, List<?> list) {
        System.out.println(label + " " + list + " size=" + list.size());
    }

    static int sumAll(List<Integer> values) {
        int total = 0;
        for (int v : values) {
            total += v;
        }
        return total;
    }

    public static void main(String[] args) {
        List<Integer> numbers = new ArrayList<>();
        for (int i = 1; i <= 10; i++) {
            numbers.add(i * i % 17);
        }
        show("squares mod 17:", numbers);
        System.out.println("sum: " + sumAll(numbers));

        Collections.sort(numbers);
        show("sorted:", numbers);
        Collections.reverse(numbers);
        show("reversed:", numbers);
        System.out.println("max: " + Collections.max(numbers) + " min: " + Collections.min(numbers));
        System.out.println("frequency of 4: " + Collections.frequency(numbers, 4));

        numbers.set(0, 100);
        numbers.remove(Integer.valueOf(4));
        numbers.remove(0);
        show("edited:", numbers);
        System.out.println("indexOf 9: " + numbers.indexOf(9) + ", lastIndexOf 9: " + numbers.lastIndexOf(9));
        System.out.println("contains 16? " + numbers.contains(16));
        List<Integer> middle = numbers.subList(2, 5);
        System.out.println("subList: " + middle);
        Collections.swap(numbers, 0, numbers.size() - 1);
        show("swapped:", numbers);

        Iterator<Integer> it = numbers.iterator();
        while (it.hasNext()) {
            if (it.next() % 2 == 0) {
                it.remove();
            }
        }
        show("odd only:", numbers);

        LinkedList<String> queue = new LinkedList<>();
        queue.add("alpha");
        queue.addFirst("zero");
        queue.addLast("omega");
        queue.add(1, "beta");
        show("linked:", queue);
        System.out.println("first: " + queue.getFirst() + ", last: " + queue.getLast());
        System.out.println("polled: " + queue.poll() + " " + queue.pollLast() + " " + queue.size());

        ListIterator<String> li = queue.listIterator();
        while (li.hasNext()) {
            int index = li.nextIndex();
            String word = li.next();
            li.set(index + ":" + word.toUpperCase());
        }
        show("upper-cased:", queue);

        List<String> fixed = List.of("x", "y", "z");
        System.out.println("fixed get(1): " + fixed.get(1));
        try {
            fixed.add("w");
            System.out.println("added?");
        } catch (UnsupportedOperationException e) {
            System.out.println("cannot add to List.of");
        }

        List<String> view = Collections.unmodifiableList(new ArrayList<>(queue));
        try {
            view.set(0, "nope");
        } catch (UnsupportedOperationException e) {
            System.out.println("view is read-only, still " + view.get(0));
        }

        List<List<Integer>> nested = new ArrayList<>();
        for (int row = 0; row < 3; row++) {
            List<Integer> line = new ArrayList<>();
            for (int col = 0; col <= row; col++) {
                line.add(row * 10 + col);
            }
            nested.add(line);
        }
        show("nested:", nested);
        System.out.println("nested equals: " + nested.get(1).equals(List.of(10, 11)));
    }
}
