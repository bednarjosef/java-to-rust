public class BoxedIncrements {
    public static void main(String[] args) {
        Integer x = 5;
        x++;
        int y = x++;
        int z = ++x;
        Short s = (short) 3;
        s++;
        s = (short) (s + 2);
        Byte b = (byte) 127;
        b++;
        Character c = (char) 65;
        c++;
        Character max = (char) 65535;
        max++;
        Float f = 1.5f;
        f++;
        f *= 2;
        Long l = 9L;
        l += 1;
        System.out.println(x + " " + y + " " + z + " " + s + " " + b + " " + c + " " + (int) max + " " + f + " " + l);
    }
}
