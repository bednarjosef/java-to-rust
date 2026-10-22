// Object's toString and hashCode named by a method reference over a stream of boxes, as
// Runtime.Version's toString joins its numbers: each is the box's own.
import java.util.List;
import java.util.stream.Collectors;

public class ObjectMethodsOfBoxes {
    public static void main(String[] args) {
        List<Integer> numbers = List.of(26, 0, 2, 1);
        System.out.println(numbers.stream().map(Object::toString).collect(Collectors.joining(".")));
        System.out.println(numbers.stream().map(Object::hashCode).collect(Collectors.toList()));
        List<Long> longs = List.of(1L << 40, -3L);
        System.out.println(longs.stream().map(Object::hashCode).collect(Collectors.toList()));
    }
}
