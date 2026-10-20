public class NegatedCharCast {
    static int sInt = 1;
    public static void main(String[] args) {
        sInt *= ((char)((-(3))));
        System.out.println(sInt);
    }
}
