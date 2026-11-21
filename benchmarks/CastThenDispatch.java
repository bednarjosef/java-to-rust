// Calls on the result of a cast, as the Minecraft server's random tick asks each block state's
// fluid whether it ticks: `FluidState.getType()` returns the state's owner, a field of a type
// variable, which javac casts to `Fluid`, and the call on it dispatches among Fluid's classes,
// between which abstract classes (`FlowingFluid`, `LavaFluid`, `WaterFluid`) are numbered. The
// owner field also holds blocks, so the cast cannot be dropped. Also a cast written as the
// receiver of a call, `((Shape) o).sides()`, over a tree with abstract classes in it.
public class CastThenDispatch {
    static class Holder<O> {
        final O owner;
        Holder(O owner) { this.owner = owner; }
    }

    static final class FluidState extends Holder<Fluid> {
        final int level;
        FluidState(Fluid fluid, int level) { super(fluid); this.level = level; }
        Fluid getType() { return this.owner; }
        boolean ticks() { return getType().ticks(); }
        int amount() { return getType().amount(this); }
    }

    static final class BlockState extends Holder<Block> {
        BlockState(Block block) { super(block); }
        Block getBlock() { return this.owner; }
    }

    abstract static class Fluid {
        abstract boolean ticks();
        abstract int amount(FluidState state);
    }
    static final class EmptyFluid extends Fluid {
        boolean ticks() { return false; }
        int amount(FluidState state) { return 0; }
    }
    abstract static class FlowingFluid extends Fluid {
        int amount(FluidState state) { return state.level; }
    }
    abstract static class LavaFluid extends FlowingFluid {
        boolean ticks() { return true; }
    }
    static final class LavaFlowing extends LavaFluid { }
    static final class LavaSource extends LavaFluid {
        int amount(FluidState state) { return 8; }
    }
    abstract static class WaterFluid extends FlowingFluid {
        boolean ticks() { return false; }
    }
    static final class WaterFlowing extends WaterFluid { }
    static final class WaterSource extends WaterFluid {
        int amount(FluidState state) { return 8; }
    }

    abstract static class Block { abstract int light(); }
    static final class Stone extends Block { int light() { return 0; } }
    static final class Torch extends Block { int light() { return 14; } }

    abstract static class Shape { abstract int sides(); }
    abstract static class Polygon extends Shape { }
    static final class Triangle extends Polygon { int sides() { return 3; } }
    static final class Square extends Polygon { int sides() { return 4; } }
    abstract static class Round extends Shape { }
    static final class Circle extends Round { int sides() { return 0; } }
    static final class Ellipse extends Round { int sides() { return 1; } }

    static long run(FluidState[] fluids, BlockState[] blocks, Object[] shapes, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int i = 0; i < fluids.length; i++) {
                FluidState f = fluids[i];
                if (f.ticks()) {
                    sum += 1;
                }
                sum += f.amount();
                sum += blocks[i].getBlock().light();
                sum += ((Shape) shapes[i]).sides();
            }
        }
        return sum;
    }

    public static void main(String[] args) throws Exception {
        Fluid[] kinds = { new EmptyFluid(), new LavaFlowing(), new LavaSource(), new WaterFlowing(), new WaterSource() };
        Block[] blockKinds = { new Stone(), new Torch() };
        Object[] shapeKinds = { new Triangle(), new Square(), new Circle(), new Ellipse() };
        FluidState[] fluids = new FluidState[1024];
        BlockState[] blocks = new BlockState[1024];
        Object[] shapes = new Object[1024];
        for (int i = 0; i < fluids.length; i++) {
            // Mostly empty, as most blocks hold no fluid.
            Fluid kind = i % 8 < 5 ? kinds[0] : kinds[1 + (i * 7) % 4];
            fluids[i] = new FluidState(kind, i % 8);
            blocks[i] = new BlockState(blockKinds[(i * 13) % 2]);
            shapes[i] = shapeKinds[(i * 37) % 4];
        }
        long[] out = new long[2];
        Thread t = new Thread(() -> {
            for (int warm = 0; warm < 3; warm++) {
                out[1] += run(fluids, blocks, shapes, 1000);
            }
            long start = System.nanoTime();
            out[0] = run(fluids, blocks, shapes, 20000);
            System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        });
        t.start();
        t.join();
        System.out.println("cast then dispatch " + out[0] + " " + out[1]);
    }
}
