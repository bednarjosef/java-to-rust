// Fields read twice in a run of statements, first in the value of a compound assignment to an
// array element or to a boxed number, which the translation computes in a closure: the first
// read cannot set a local there, so the field is read where it is written.
public class FieldReadsInElementUpdates {
    int position = 10;
    int start = 3;
    int size = 2;
    float[] sums = new float[2];
    Integer total = 5;

    void step() {
        sums[0] += position - start * size;
        sums[1] -= start * size;
        total += size;
        total -= start;
        position += start + size;
    }

    public static void main(String[] args) {
        FieldReadsInElementUpdates f = new FieldReadsInElementUpdates();
        f.step();
        f.step();
        System.out.println(f.sums[0] + " " + f.sums[1] + " " + f.total + " " + f.position);
    }
}
