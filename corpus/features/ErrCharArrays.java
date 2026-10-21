public class ErrCharArrays {
    public static void main(String[] args) {
        char[] word = {'o', 'k', ' ', 'é', '世'};
        char[] part = {'a', 'b', 'c', 'd'};
        // Standard error is not compared, but the text is printed, and what System.out printed
        // before it must come out first.
        System.out.print("first ");
        System.err.print(word);
        System.err.println(part);
        System.err.println(word);
        System.err.print(new char[0]);
        System.err.println(new char[] {'!'});
        System.out.println(word);
        System.out.println("last");
        System.exit(3);
    }
}
