public class IdiomaticBooleans {
    static boolean divisibleByThree(int n) {
        return n % 3 == 0;
    }

    public static void main(String[] args) {
        boolean yes = true;
        boolean no = false;
        int count = 0;
        for (int i = 0; i < 8; i++) {
            boolean odd = i % 2 == 1;
            if (odd == true) count += 1;
            if (odd != false) count += 10;
            if (divisibleByThree(i) == false) count += 100;
            if (false == divisibleByThree(i)) count += 1000;
            if (true != odd) count += 10000;
            if (no != yes) count += 100000;
            System.out.println(i + " " + (odd == true) + " " + (odd != true) + " " + (divisibleByThree(i) == false)
                + " " + (divisibleByThree(i) != true) + " " + (false == odd) + " " + (no == odd));
        }
        System.out.println(count);
        System.out.println((yes == true) + " " + (no == false) + " " + (yes != no) + " " + (no == yes));
    }
}
