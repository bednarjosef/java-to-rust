// Type tests of objects of many classes held as Objects, as the Minecraft server's casts to
// List and its entity lookups ask them: `instanceof` an interface whose classes are many runs of
// class numbers (each implementing class shares a base class with one that does not), and
// `Class.isInstance` of a class with subclasses and of an interface.
public class TypeTests {
    interface Shape { int sides(); }

    abstract static class Entity { abstract int tag(); }

    abstract static class B00 extends Entity { }
    static final class S00 extends B00 implements Shape {
        int tag() { return 0; }
        public int sides() { return 3; }
    }
    static final class P00 extends B00 { int tag() { return 100; } }
    abstract static class B01 extends Entity { }
    static final class S01 extends B01 implements Shape {
        int tag() { return 1; }
        public int sides() { return 4; }
    }
    static final class P01 extends B01 { int tag() { return 101; } }
    abstract static class B02 extends Entity { }
    static final class S02 extends B02 implements Shape {
        int tag() { return 2; }
        public int sides() { return 5; }
    }
    static final class P02 extends B02 { int tag() { return 102; } }
    abstract static class B03 extends Entity { }
    static final class S03 extends B03 implements Shape {
        int tag() { return 3; }
        public int sides() { return 6; }
    }
    static final class P03 extends B03 { int tag() { return 103; } }
    abstract static class B04 extends Entity { }
    static final class S04 extends B04 implements Shape {
        int tag() { return 4; }
        public int sides() { return 7; }
    }
    static final class P04 extends B04 { int tag() { return 104; } }
    abstract static class B05 extends Entity { }
    static final class S05 extends B05 implements Shape {
        int tag() { return 5; }
        public int sides() { return 8; }
    }
    static final class P05 extends B05 { int tag() { return 105; } }
    abstract static class B06 extends Entity { }
    static final class S06 extends B06 implements Shape {
        int tag() { return 6; }
        public int sides() { return 3; }
    }
    static final class P06 extends B06 { int tag() { return 106; } }
    abstract static class B07 extends Entity { }
    static final class S07 extends B07 implements Shape {
        int tag() { return 7; }
        public int sides() { return 4; }
    }
    static final class P07 extends B07 { int tag() { return 107; } }
    abstract static class B08 extends Entity { }
    static final class S08 extends B08 implements Shape {
        int tag() { return 8; }
        public int sides() { return 5; }
    }
    static final class P08 extends B08 { int tag() { return 108; } }
    abstract static class B09 extends Entity { }
    static final class S09 extends B09 implements Shape {
        int tag() { return 9; }
        public int sides() { return 6; }
    }
    static final class P09 extends B09 { int tag() { return 109; } }
    abstract static class B10 extends Entity { }
    static final class S10 extends B10 implements Shape {
        int tag() { return 10; }
        public int sides() { return 7; }
    }
    static final class P10 extends B10 { int tag() { return 110; } }
    abstract static class B11 extends Entity { }
    static final class S11 extends B11 implements Shape {
        int tag() { return 11; }
        public int sides() { return 8; }
    }
    static final class P11 extends B11 { int tag() { return 111; } }
    abstract static class B12 extends Entity { }
    static final class S12 extends B12 implements Shape {
        int tag() { return 12; }
        public int sides() { return 3; }
    }
    static final class P12 extends B12 { int tag() { return 112; } }
    abstract static class B13 extends Entity { }
    static final class S13 extends B13 implements Shape {
        int tag() { return 13; }
        public int sides() { return 4; }
    }
    static final class P13 extends B13 { int tag() { return 113; } }
    abstract static class B14 extends Entity { }
    static final class S14 extends B14 implements Shape {
        int tag() { return 14; }
        public int sides() { return 5; }
    }
    static final class P14 extends B14 { int tag() { return 114; } }
    abstract static class B15 extends Entity { }
    static final class S15 extends B15 implements Shape {
        int tag() { return 15; }
        public int sides() { return 6; }
    }
    static final class P15 extends B15 { int tag() { return 115; } }
    abstract static class B16 extends Entity { }
    static final class S16 extends B16 implements Shape {
        int tag() { return 16; }
        public int sides() { return 7; }
    }
    static final class P16 extends B16 { int tag() { return 116; } }
    abstract static class B17 extends Entity { }
    static final class S17 extends B17 implements Shape {
        int tag() { return 17; }
        public int sides() { return 8; }
    }
    static final class P17 extends B17 { int tag() { return 117; } }
    abstract static class B18 extends Entity { }
    static final class S18 extends B18 implements Shape {
        int tag() { return 18; }
        public int sides() { return 3; }
    }
    static final class P18 extends B18 { int tag() { return 118; } }
    abstract static class B19 extends Entity { }
    static final class S19 extends B19 implements Shape {
        int tag() { return 19; }
        public int sides() { return 4; }
    }
    static final class P19 extends B19 { int tag() { return 119; } }
    abstract static class B20 extends Entity { }
    static final class S20 extends B20 implements Shape {
        int tag() { return 20; }
        public int sides() { return 5; }
    }
    static final class P20 extends B20 { int tag() { return 120; } }
    abstract static class B21 extends Entity { }
    static final class S21 extends B21 implements Shape {
        int tag() { return 21; }
        public int sides() { return 6; }
    }
    static final class P21 extends B21 { int tag() { return 121; } }
    abstract static class B22 extends Entity { }
    static final class S22 extends B22 implements Shape {
        int tag() { return 22; }
        public int sides() { return 7; }
    }
    static final class P22 extends B22 { int tag() { return 122; } }
    abstract static class B23 extends Entity { }
    static final class S23 extends B23 implements Shape {
        int tag() { return 23; }
        public int sides() { return 8; }
    }
    static final class P23 extends B23 { int tag() { return 123; } }
    abstract static class B24 extends Entity { }
    static final class S24 extends B24 implements Shape {
        int tag() { return 24; }
        public int sides() { return 3; }
    }
    static final class P24 extends B24 { int tag() { return 124; } }
    abstract static class B25 extends Entity { }
    static final class S25 extends B25 implements Shape {
        int tag() { return 25; }
        public int sides() { return 4; }
    }
    static final class P25 extends B25 { int tag() { return 125; } }
    abstract static class B26 extends Entity { }
    static final class S26 extends B26 implements Shape {
        int tag() { return 26; }
        public int sides() { return 5; }
    }
    static final class P26 extends B26 { int tag() { return 126; } }
    abstract static class B27 extends Entity { }
    static final class S27 extends B27 implements Shape {
        int tag() { return 27; }
        public int sides() { return 6; }
    }
    static final class P27 extends B27 { int tag() { return 127; } }
    abstract static class B28 extends Entity { }
    static final class S28 extends B28 implements Shape {
        int tag() { return 28; }
        public int sides() { return 7; }
    }
    static final class P28 extends B28 { int tag() { return 128; } }
    abstract static class B29 extends Entity { }
    static final class S29 extends B29 implements Shape {
        int tag() { return 29; }
        public int sides() { return 8; }
    }
    static final class P29 extends B29 { int tag() { return 129; } }
    abstract static class B30 extends Entity { }
    static final class S30 extends B30 implements Shape {
        int tag() { return 30; }
        public int sides() { return 3; }
    }
    static final class P30 extends B30 { int tag() { return 130; } }
    abstract static class B31 extends Entity { }
    static final class S31 extends B31 implements Shape {
        int tag() { return 31; }
        public int sides() { return 4; }
    }
    static final class P31 extends B31 { int tag() { return 131; } }
    abstract static class B32 extends Entity { }
    static final class S32 extends B32 implements Shape {
        int tag() { return 32; }
        public int sides() { return 5; }
    }
    static final class P32 extends B32 { int tag() { return 132; } }
    abstract static class B33 extends Entity { }
    static final class S33 extends B33 implements Shape {
        int tag() { return 33; }
        public int sides() { return 6; }
    }
    static final class P33 extends B33 { int tag() { return 133; } }
    abstract static class B34 extends Entity { }
    static final class S34 extends B34 implements Shape {
        int tag() { return 34; }
        public int sides() { return 7; }
    }
    static final class P34 extends B34 { int tag() { return 134; } }
    abstract static class B35 extends Entity { }
    static final class S35 extends B35 implements Shape {
        int tag() { return 35; }
        public int sides() { return 8; }
    }
    static final class P35 extends B35 { int tag() { return 135; } }
    abstract static class B36 extends Entity { }
    static final class S36 extends B36 implements Shape {
        int tag() { return 36; }
        public int sides() { return 3; }
    }
    static final class P36 extends B36 { int tag() { return 136; } }
    abstract static class B37 extends Entity { }
    static final class S37 extends B37 implements Shape {
        int tag() { return 37; }
        public int sides() { return 4; }
    }
    static final class P37 extends B37 { int tag() { return 137; } }
    abstract static class B38 extends Entity { }
    static final class S38 extends B38 implements Shape {
        int tag() { return 38; }
        public int sides() { return 5; }
    }
    static final class P38 extends B38 { int tag() { return 138; } }
    abstract static class B39 extends Entity { }
    static final class S39 extends B39 implements Shape {
        int tag() { return 39; }
        public int sides() { return 6; }
    }
    static final class P39 extends B39 { int tag() { return 139; } }
    static final class R00 implements Runnable { public void run() { } }
    static final class R01 implements Runnable { public void run() { } }
    static final class R02 implements Runnable { public void run() { } }
    static final class R03 implements Runnable { public void run() { } }
    static final class R04 implements Runnable { public void run() { } }
    static final class R05 implements Runnable { public void run() { } }
    static final class R06 implements Runnable { public void run() { } }
    static final class R07 implements Runnable { public void run() { } }
    static final class R08 implements Runnable { public void run() { } }
    static final class R09 implements Runnable { public void run() { } }
    static final class R10 implements Runnable { public void run() { } }
    static final class R11 implements Runnable { public void run() { } }
    static final class R12 implements Runnable { public void run() { } }
    static final class R13 implements Runnable { public void run() { } }
    static final class R14 implements Runnable { public void run() { } }
    static final class R15 implements Runnable { public void run() { } }
    static final class R16 implements Runnable { public void run() { } }
    static final class R17 implements Runnable { public void run() { } }
    static final class R18 implements Runnable { public void run() { } }
    static final class R19 implements Runnable { public void run() { } }
    static final class R20 implements Runnable { public void run() { } }
    static final class R21 implements Runnable { public void run() { } }
    static final class R22 implements Runnable { public void run() { } }
    static final class R23 implements Runnable { public void run() { } }
    static final class R24 implements Runnable { public void run() { } }
    static final class R25 implements Runnable { public void run() { } }
    static final class R26 implements Runnable { public void run() { } }
    static final class R27 implements Runnable { public void run() { } }
    static final class R28 implements Runnable { public void run() { } }
    static final class R29 implements Runnable { public void run() { } }
    static final class R30 implements Runnable { public void run() { } }
    static final class R31 implements Runnable { public void run() { } }
    static final class R32 implements Runnable { public void run() { } }
    static final class R33 implements Runnable { public void run() { } }
    static final class R34 implements Runnable { public void run() { } }
    static final class R35 implements Runnable { public void run() { } }
    static final class R36 implements Runnable { public void run() { } }
    static final class R37 implements Runnable { public void run() { } }
    static final class R38 implements Runnable { public void run() { } }
    static final class R39 implements Runnable { public void run() { } }

    static long run(Object[] objects, Class<?> wanted, Class<?> shape, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (Object o : objects) {
                if (o instanceof Shape s) {
                    sum += s.sides();
                }
                if (wanted.isInstance(o)) {
                    sum += 3;
                }
                if (shape.isInstance(o)) {
                    sum += 5;
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) throws Exception {
        Object[] all = { new S00(), new P00(), new S01(), new P01(), new S02(), new P02(), new S03(), new P03(), new S04(), new P04(), new S05(), new P05(), new S06(), new P06(), new S07(), new P07(), new S08(), new P08(), new S09(), new P09(), new S10(), new P10(), new S11(), new P11(), new S12(), new P12(), new S13(), new P13(), new S14(), new P14(), new S15(), new P15(), new S16(), new P16(), new S17(), new P17(), new S18(), new P18(), new S19(), new P19(), new S20(), new P20(), new S21(), new P21(), new S22(), new P22(), new S23(), new P23(), new S24(), new P24(), new S25(), new P25(), new S26(), new P26(), new S27(), new P27(), new S28(), new P28(), new S29(), new P29(), new S30(), new P30(), new S31(), new P31(), new S32(), new P32(), new S33(), new P33(), new S34(), new P34(), new S35(), new P35(), new S36(), new P36(), new S37(), new P37(), new S38(), new P38(), new S39(), new P39(), new R00(), new R01(), new R02(), new R03(), new R04(), new R05(), new R06(), new R07(), new R08(), new R09(), new R10(), new R11(), new R12(), new R13(), new R14(), new R15(), new R16(), new R17(), new R18(), new R19(), new R20(), new R21(), new R22(), new R23(), new R24(), new R25(), new R26(), new R27(), new R28(), new R29(), new R30(), new R31(), new R32(), new R33(), new R34(), new R35(), new R36(), new R37(), new R38(), new R39() };
        Object[] objects = new Object[1024];
        for (int i = 0; i < objects.length; i++) {
            objects[i] = all[(i * 37) % all.length];
        }
        Class<?> wanted = args.length > 0 ? Object.class : Entity.class;
        Class<?> shape = args.length > 0 ? Runnable.class : Shape.class;
        long[] out = new long[2];
        Thread t = new Thread(() -> {
            for (int warm = 0; warm < 3; warm++) {
                out[1] += run(objects, wanted, shape, 1000);
            }
            long start = System.nanoTime();
            out[0] = run(objects, wanted, shape, 30000);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        t.start();
        t.join();
        System.out.println("type tests " + out[0] + " " + out[1]);
    }
}
