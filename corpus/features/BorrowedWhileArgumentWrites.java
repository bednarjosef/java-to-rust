public class BorrowedWhileArgumentWrites {
    static char[] shared;

    static int squeeze(char[] path) {
        int p = 2, q = 4;
        while (q < path.length) path[p++] = path[q++];
        return p;
    }

    static int upper(byte[] data) {
        for (int i = 0; i < data.length; i++) data[i] = (byte) (data[i] - 32);
        return 3;
    }

    public static void main(String[] args) throws Exception {
        char[] path = "ab/./cd".toCharArray();
        Thread t = new Thread(() -> shared = path);
        t.start();
        t.join();
        System.out.println(new String(path, 0, squeeze(path)));
        char[] single = "xy/./z".toCharArray();
        System.out.println(new String(single, 0, squeeze(single)));
        char[] other = "uvwxyz".toCharArray();
        System.out.println(new String(other, 0, (other = "123456".toCharArray()).length - 3));
        System.out.println(new String(other, 0, 2));
        byte[] bytes = "abcdef".getBytes();
        System.out.println(new String(bytes, 0, upper(bytes)));
        System.out.println(String.valueOf(single, 0, squeeze(single)));
    }
}
