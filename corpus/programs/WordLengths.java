public class WordLengths {
    static Integer lookup(String key) {
        if (key.equals("ten")) {
            return 10;
        }
        if (key.equals("none")) {
            return null;
        }
        return key.length();
    }

    static int total(String[] parts) {
        Integer sum = 0;
        for (String part : parts) {
            if (Character.isDigit(part.charAt(0))) {
                sum += Integer.parseInt(part);
            }
            Integer n = lookup(part);
            if (n != null) {
                sum += n;
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        String[] words = {"one", "two", "three", "ten", "none", "12"};
        int count = 0;
        Integer longest = 0;
        for (String word : words) {
            Integer len = word.length();
            count++;
            if (len > longest) {
                longest = len;
            }
            System.out.println(word + " " + len + " " + lookup(word));
        }
        System.out.println(count + " " + longest + " " + total(words));
        Integer missing = lookup("none");
        System.out.println("missing=" + missing);
        System.out.println(missing + 1);
    }
}
