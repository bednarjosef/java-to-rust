public class StringArrayFieldInitializer {
    static class Bag {
        String[] items = new String[2];
        int count;

        void add(String item) {
            items[count] = item;
            count++;
        }
    }

    public static void main(String[] args) {
        Bag b = new Bag();
        b.add("pen");
        System.out.println(b.items[0] + " " + b.items[1] + " " + b.count);
    }
}
