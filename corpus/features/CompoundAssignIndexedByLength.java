// A compound assignment or increment of an element whose index reads the same array, such as
// its length.
public class CompoundAssignIndexedByLength {
    static int sInt = 7;

    public static void main(String[] args) {
        float[] weights = new float[5];
        weights[Math.floorMod(sInt, weights.length)] += 1.5f;
        weights[Math.floorMod(sInt, weights.length)] *= 3.0;
        int[] counts = {1, 2, 3};
        counts[counts.length - 1]++;
        --counts[Math.floorMod(-sInt, counts.length)];
        counts[counts[0]] <<= sInt;
        counts[Math.floorMod(sInt * 1000000, counts.length)] -= 2147483647;
        byte[] small = new byte[4];
        small[small.length - 1] -= 130;
        System.out.println(weights[2] + " " + counts[0] + " " + counts[1] + " " + counts[2] + " " + small[3]);
        sInt = 40;
        counts[Math.floorMod(sInt, counts.length)] /= 2;
        System.out.println(counts[1]);
    }
}
