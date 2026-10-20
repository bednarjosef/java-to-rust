import java.util.Arrays;

public class IndexedIncrementReadsArray {
    static char sChar = '~';
    static double[] aDouble = new double[6];
    public static void main(String[] args) {
        int[] a5 = new int[] {sChar};
        switch (0) {
            case 3: {
                switch (Arrays.toString(aDouble)) {
                    case "": {
                        a5[Math.floorMod((0), a5.length)]++;
                    }
                }
            }
        }
        int[] counts = new int[] {sChar, 2};
        counts[Math.floorMod(sChar, counts.length)]++;
        counts[counts.length - 1]--;
        System.out.println(a5[0] + " " + counts[0] + " " + counts[1]);
    }
}
