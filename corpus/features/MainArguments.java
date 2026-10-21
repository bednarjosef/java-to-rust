import java.util.Arrays;

public class MainArguments {
    static String[] kept;

    public static void main(String[] args) {
        System.out.println("count: " + args.length);
        for (int i = 0; i < args.length; i++) {
            System.out.println("arg " + i + ": [" + args[i] + "] length " + args[i].length());
        }
        for (String arg : args) {
            System.out.print(arg.isEmpty() ? "(empty) " : arg.toUpperCase() + " ");
        }
        System.out.println();
        int doubled = Integer.parseInt(args[0]) * 2;
        System.out.println("doubled: " + doubled);

        kept = args;
        args[0] = "changed";
        System.out.println("kept: " + kept[0] + " " + kept.length);
        System.out.println("described: " + describe(args));
        System.out.println(Arrays.toString(kept));
    }

    static String describe(String[] words) {
        return words.length + " " + words[1];
    }
}
