public class BoxedArrays {
    public static void main(String[] args) {
        Integer[] counts = new Integer[4];
        counts[1] = 5;
        counts[2] = counts[1] + 1;
        int total = 0;
        for (Integer c : counts) if (c != null) total += c;
        System.out.println(total + " " + counts[0] + " " + counts[2]);
        Integer[] lit = {1, 2, null};
        System.out.println(lit[0] + lit[1]);
        System.out.println(lit[2] == null);
        Double[] ds = new Double[2];
        ds[0] = 1.5;
        System.out.println(ds[0] + " " + ds[1]);
        int bad = counts[3];
    }
}
