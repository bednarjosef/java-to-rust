public class LoneSurrogateCompareTo {
    public static void main(String[] args) {
        String tail = "😀x".substring(1);
        System.out.println(tail.length() + " " + "<".compareTo(tail) + " " + "日".compareTo(tail));
    }
}
