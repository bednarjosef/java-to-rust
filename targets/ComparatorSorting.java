// Features: a Person class with getters, Comparator.comparing with method references,
// thenComparing, reversed, nullsFirst, Comparator.comparingInt, an anonymous Comparator class,
// Collections.sort and List.sort, Arrays.sort on an Integer[] with a lambda, a stable-sort check,
// equals/hashCode on a user class used in a HashSet, and a binary search on a sorted list.
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;

public class ComparatorSorting {
    static class Person {
        private final String first;
        private final String last;
        private final int age;

        Person(String first, String last, int age) {
            this.first = first;
            this.last = last;
            this.age = age;
        }

        String getFirst() {
            return first;
        }

        String getLast() {
            return last;
        }

        int getAge() {
            return age;
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) {
                return true;
            }
            if (o == null || getClass() != o.getClass()) {
                return false;
            }
            Person p = (Person) o;
            return age == p.age && first.equals(p.first) && last.equals(p.last);
        }

        @Override
        public int hashCode() {
            return Objects.hash(first, last, age);
        }

        @Override
        public String toString() {
            return first + " " + last + " (" + age + ")";
        }
    }

    static void print(String title, List<Person> people) {
        System.out.println(title);
        for (Person p : people) {
            System.out.println("  " + p);
        }
    }

    public static void main(String[] args) {
        List<Person> people = new ArrayList<>();
        people.add(new Person("Grace", "Hopper", 85));
        people.add(new Person("Alan", "Turing", 41));
        people.add(new Person("Ada", "Lovelace", 36));
        people.add(new Person("Edsger", "Dijkstra", 72));
        people.add(new Person("Barbara", "Liskov", 85));
        people.add(new Person("Alan", "Kay", 83));

        List<Person> byLast = new ArrayList<>(people);
        byLast.sort(Comparator.comparing(Person::getLast));
        print("by last name:", byLast);

        List<Person> byAgeThenName = new ArrayList<>(people);
        byAgeThenName.sort(Comparator.comparingInt(Person::getAge)
                .thenComparing(Person::getFirst)
                .thenComparing(Person::getLast));
        print("by age, then first, then last:", byAgeThenName);

        List<Person> oldestFirst = new ArrayList<>(people);
        Collections.sort(oldestFirst, Comparator.comparing(Person::getAge).reversed());
        print("oldest first (stable within equal ages):", oldestFirst);

        Comparator<Person> byFirstLength = new Comparator<Person>() {
            @Override
            public int compare(Person a, Person b) {
                int diff = a.getFirst().length() - b.getFirst().length();
                return diff != 0 ? diff : a.getFirst().compareTo(b.getFirst());
            }
        };
        List<Person> byFirst = new ArrayList<>(people);
        byFirst.sort(byFirstLength);
        print("by first-name length:", byFirst);

        List<String> maybe = new ArrayList<>(Arrays.asList("pear", null, "apple", null, "fig"));
        maybe.sort(Comparator.nullsFirst(Comparator.naturalOrder()));
        System.out.println("nulls first: " + maybe);
        maybe.sort(Comparator.nullsLast(Comparator.reverseOrder()));
        System.out.println("nulls last: " + maybe);

        Integer[] numbers = {15, -3, 42, 7, 0, 7};
        Arrays.sort(numbers, (a, b) -> Integer.compare(Math.abs(a), Math.abs(b)));
        System.out.println("by absolute value: " + Arrays.toString(numbers));
        Arrays.sort(numbers, Collections.reverseOrder());
        System.out.println("descending: " + Arrays.toString(numbers));

        List<Integer> sortedNumbers = new ArrayList<>(Arrays.asList(numbers));
        Collections.sort(sortedNumbers);
        System.out.println("search 15: " + Collections.binarySearch(sortedNumbers, 15));
        System.out.println("search 16: " + Collections.binarySearch(sortedNumbers, 16));

        Set<Person> unique = new HashSet<>(people);
        unique.add(new Person("Alan", "Turing", 41));
        System.out.println("unique people: " + unique.size());
        System.out.println("contains Ada: " + unique.contains(new Person("Ada", "Lovelace", 36)));
    }
}
