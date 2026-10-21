public class MainArgumentsPlain {
    public static void main(String[] args) {
        int total = 0;
        for (String arg : args) {
            total += arg.length();
        }
        System.out.println("arguments: " + args.length + ", characters: " + total);
        if (args.length > 1) {
            System.out.println("second: " + args[1].toUpperCase());
        }
        System.out.println("last is null: " + (args[args.length - 1] == null));
    }
}
