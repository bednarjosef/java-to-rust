// The static and default methods of Comparator, on method references and lambdas. (reversed()
// reaches java.util.Collections, whose empty collections are not translated yet.)
import java.util.Comparator;

public class Comparators {
    static class Person {
        final String name;
        final int age;

        Person(String name, int age) {
            this.name = name;
            this.age = age;
        }

        String name() { return name; }

        int age() { return age; }
    }

    static int sign(int x) {
        return Integer.signum(x);
    }

    public static void main(String[] args) {
        Person ann = new Person("Ann", 30);
        Person bob = new Person("Bob", 25);
        Person amy = new Person("Ann", 20);
        Comparator<Person> byName = Comparator.comparing(Person::name);
        Comparator<Person> byAge = Comparator.comparingInt(Person::age);
        System.out.println(sign(byName.compare(ann, bob)) + " " + sign(byAge.compare(ann, bob)));
        Comparator<Person> both = byName.thenComparing(byAge);
        System.out.println(sign(both.compare(ann, amy)));
        Comparator<Person> thenAge = byName.thenComparingInt(Person::age);
        System.out.println(sign(thenAge.compare(amy, ann)));
        Comparator<String> natural = Comparator.naturalOrder();
        System.out.println(sign(natural.compare("a", "b")));
        Comparator<String> nullsFirst = Comparator.nullsFirst(natural);
        System.out.println(sign(nullsFirst.compare(null, "a")) + " " + sign(nullsFirst.compare("b", "a")));
        Comparator<Person> lambda = (p, q) -> p.age - q.age;
        System.out.println(sign(lambda.thenComparing(Person::name).compare(ann, bob)));
    }
}
