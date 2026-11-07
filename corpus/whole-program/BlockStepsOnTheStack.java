// The block stepping of an entity's move, after the Minecraft server's
// `BlockGetter.forEachBlockIntersectedBetween`: every object it makes stays on the stack.
// - `BlockPos.betweenCornersInDirection(AABB, Vec3)` is a factory that takes its corners from two
//   factories and returns a third's object; it is a factory whatever order the analysis finds
//   the three in.
// - The iterator of `betweenCornersInDirection` has 22 fields, most of them numbers, which fit a
//   slot.
// - The stepping method makes ten objects, more than eight, all small.
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;
import java.util.Optional;

class Vec3 {
  public static final Vec3 ZERO = new Vec3(0, 0, 0);
  public final double x, y, z;
  Vec3(double x, double y, double z) { this.x = x; this.y = y; this.z = z; }
  double x() { return x; } double y() { return y; } double z() { return z; }
  Vec3 subtract(Vec3 o) { return new Vec3(x - o.x, y - o.y, z - o.z); }
  Vec3 scale(double s) { return new Vec3(x * s, y * s, z * s); }
  double lengthSqr() { return x * x + y * y + z * z; }
  double get(Axis a) { return a.choose(x, y, z); }
}
class Vec3i { final int x, y, z; Vec3i(int x, int y, int z) { this.x = x; this.y = y; this.z = z; } int getX() { return x; } int getY() { return y; } int getZ() { return z; } }
class BlockPos extends Vec3i {
  BlockPos(int x, int y, int z) { super(x, y, z); }
  long asLong() { return ((long) getX() & 0x3FFFFFFL) << 38 | ((long) getY() & 0xFFFL) | ((long) getZ() & 0x3FFFFFFL) << 12; }
  static Iterable<BlockPos> betweenClosed(AABB box) {
    return betweenClosed((int) Math.floor(box.minX), (int) Math.floor(box.minY), (int) Math.floor(box.minZ), (int) Math.floor(box.maxX), (int) Math.floor(box.maxY), (int) Math.floor(box.maxZ));
  }
  static Iterable<BlockPos> betweenClosed(int minX, int minY, int minZ, int maxX, int maxY, int maxZ) {
    int width = maxX - minX + 1, height = maxY - minY + 1, depth = maxZ - minZ + 1;
    int end = width * height * depth;
    return () -> new AbstractIterator<BlockPos>() {
      private final Mutable cursor = new Mutable();
      private int index;
      protected BlockPos computeNext() {
        if (index == end) return endOfData();
        int x = index % width, slice = index / width, y = slice % height, z = slice / height;
        index++;
        return cursor.set(minX + x, minY + y, minZ + z);
      }
    };
  }
  static Iterable<BlockPos> betweenCornersInDirection(AABB aabb, Vec3 direction) {
    Vec3 minCorner = aabb.getMinPosition();
    int fx = (int) Math.floor(minCorner.x()), fy = (int) Math.floor(minCorner.y()), fz = (int) Math.floor(minCorner.z());
    Vec3 maxCorner = aabb.getMaxPosition();
    int sx = (int) Math.floor(maxCorner.x()), sy = (int) Math.floor(maxCorner.y()), sz = (int) Math.floor(maxCorner.z());
    return betweenCornersInDirection(fx, fy, fz, sx, sy, sz, direction);
  }
  static Iterable<BlockPos> betweenCornersInDirection(int ax, int ay, int az, int bx, int by, int bz, Vec3 direction) {
    int minX = Math.min(ax, bx), minY = Math.min(ay, by), minZ = Math.min(az, bz);
    int maxX = Math.max(ax, bx), maxY = Math.max(ay, by), maxZ = Math.max(az, bz);
    int diffX = maxX - minX, diffY = maxY - minY, diffZ = maxZ - minZ;
    int startX = direction.x >= 0.0 ? minX : maxX, startY = direction.y >= 0.0 ? minY : maxY, startZ = direction.z >= 0.0 ? minZ : maxZ;
    List<Axis> axes = Direction.axisStepOrder(direction);
    Axis firstAxis = axes.get(0), secondAxis = axes.get(1), thirdAxis = axes.get(2);
    Direction firstDir = direction.get(firstAxis) >= 0.0 ? firstAxis.getPositive() : firstAxis.getNegative();
    Direction secondDir = direction.get(secondAxis) >= 0.0 ? secondAxis.getPositive() : secondAxis.getNegative();
    Direction thirdDir = direction.get(thirdAxis) >= 0.0 ? thirdAxis.getPositive() : thirdAxis.getNegative();
    int firstMax = firstAxis.choose(diffX, diffY, diffZ), secondMax = secondAxis.choose(diffX, diffY, diffZ), thirdMax = thirdAxis.choose(diffX, diffY, diffZ);
    return () -> new AbstractIterator<BlockPos>() {
      private final Mutable cursor = new Mutable();
      private int firstIndex, secondIndex, thirdIndex;
      private boolean end;
      private final int firstDirX = firstDir.getStepX(), firstDirY = firstDir.getStepY(), firstDirZ = firstDir.getStepZ();
      private final int secondDirX = secondDir.getStepX(), secondDirY = secondDir.getStepY(), secondDirZ = secondDir.getStepZ();
      private final int thirdDirX = thirdDir.getStepX(), thirdDirY = thirdDir.getStepY(), thirdDirZ = thirdDir.getStepZ();
      protected BlockPos computeNext() {
        if (end) return endOfData();
        cursor.set(startX + firstDirX * firstIndex + secondDirX * secondIndex + thirdDirX * thirdIndex,
                   startY + firstDirY * firstIndex + secondDirY * secondIndex + thirdDirY * thirdIndex,
                   startZ + firstDirZ * firstIndex + secondDirZ * secondIndex + thirdDirZ * thirdIndex);
        if (thirdIndex < thirdMax) thirdIndex++;
        else if (secondIndex < secondMax) { secondIndex++; thirdIndex = 0; }
        else if (firstIndex < firstMax) { firstIndex++; thirdIndex = 0; secondIndex = 0; }
        else end = true;
        return cursor;
      }
    };
  }
  static final class Mutable extends BlockPos {
    int mx, my, mz;
    Mutable() { super(0, 0, 0); }
    Mutable set(int x, int y, int z) { mx = x; my = y; mz = z; return this; }
    int getX() { return mx; } int getY() { return my; } int getZ() { return mz; }
  }
}
enum Direction {
  DOWN(0, -1, 0), UP(0, 1, 0), NORTH(0, 0, -1), SOUTH(0, 0, 1), WEST(-1, 0, 0), EAST(1, 0, 0);
  final int sx, sy, sz;
  Direction(int sx, int sy, int sz) { this.sx = sx; this.sy = sy; this.sz = sz; }
  int getStepX() { return sx; } int getStepY() { return sy; } int getStepZ() { return sz; }
  static final List<Axis> YXZ = List.of(Axis.Y, Axis.X, Axis.Z), YZX = List.of(Axis.Y, Axis.Z, Axis.X);
  static List<Axis> axisStepOrder(Vec3 m) { return Math.abs(m.x) < Math.abs(m.z) ? YZX : YXZ; }
}
enum Axis {
  X, Y, Z;
  int choose(int x, int y, int z) { return this == X ? x : this == Y ? y : z; }
  double choose(double x, double y, double z) { return this == X ? x : this == Y ? y : z; }
  Direction getPositive() { return this == X ? Direction.EAST : this == Y ? Direction.UP : Direction.SOUTH; }
  Direction getNegative() { return this == X ? Direction.WEST : this == Y ? Direction.DOWN : Direction.NORTH; }
}
abstract class AbstractIterator<T> implements Iterator<T> {
  private enum State { READY, NOT_READY, DONE, FAILED }
  private State state = State.NOT_READY;
  private T next;
  protected abstract T computeNext();
  protected final T endOfData() { state = State.DONE; return null; }
  public final boolean hasNext() {
    switch (state) { case DONE: return false; case READY: return true; default: }
    state = State.FAILED; next = computeNext();
    if (state != State.DONE) { state = State.READY; return true; }
    return false;
  }
  public final T next() { if (!hasNext()) throw new NoSuchElementException(); state = State.NOT_READY; T result = next; next = null; return result; }
}
class AABB {
  final double minX, minY, minZ, maxX, maxY, maxZ;
  AABB(double a, double b, double c, double d, double e, double f) { minX = Math.min(a, d); minY = Math.min(b, e); minZ = Math.min(c, f); maxX = Math.max(a, d); maxY = Math.max(b, e); maxZ = Math.max(c, f); }
  Vec3 getMinPosition() { return new Vec3(minX, minY, minZ); }
  Vec3 getMaxPosition() { return new Vec3(maxX, maxY, maxZ); }
  AABB move(Vec3 p) { return new AABB(minX + p.x, minY + p.y, minZ + p.z, maxX + p.x, maxY + p.y, maxZ + p.z); }
  static Optional<Vec3> clip(double x0, double y0, double z0, double x1, double y1, double z1, Vec3 from, Vec3 to) {
    double t = (x0 - from.x) / (to.x - from.x);
    if (t >= 0 && t <= 1 && from.y <= y1) return Optional.of(new Vec3(x0, from.y + (to.y - from.y) * t, from.z + (to.z - from.z) * t));
    return Optional.empty();
  }
}
class LongSet {
  long[] keys = new long[16]; int size;
  boolean add(long k) { for (int i = 0; i < size; i++) if (keys[i] == k) return false; if (size == keys.length) keys = java.util.Arrays.copyOf(keys, size * 2); keys[size++] = k; return true; }
}
interface BlockStepVisitor { boolean visit(BlockPos pos, int iteration); }
public class BlockStepsOnTheStack {
  static boolean forEachBlockIntersectedBetween(Vec3 from, Vec3 to, AABB aabbAtTarget, BlockStepVisitor visitor) {
    Vec3 travel = to.subtract(from);
    if (travel.lengthSqr() < 1e-10) {
      for (BlockPos blockPos : BlockPos.betweenClosed(aabbAtTarget)) if (!visitor.visit(blockPos, 0)) return false;
      return true;
    } else {
      LongSet visitedBlocks = new LongSet();
      for (BlockPos blockPos : BlockPos.betweenCornersInDirection(aabbAtTarget.move(travel.scale(-1.0)), travel)) {
        if (!visitor.visit(blockPos, 0)) return false;
        visitedBlocks.add(blockPos.asLong());
      }
      int iterations = addCollisionsAlongTravel(visitedBlocks, travel, aabbAtTarget, visitor);
      if (iterations < 0) return false;
      for (BlockPos blockPos : BlockPos.betweenCornersInDirection(aabbAtTarget, travel)) {
        if (visitedBlocks.add(blockPos.asLong()) && !visitor.visit(blockPos, iterations + 1)) return false;
      }
      return true;
    }
  }
  static int addCollisionsAlongTravel(LongSet visited, Vec3 delta, AABB aabb, BlockStepVisitor visitor) {
    Vec3 toCorner = aabb.getMaxPosition();
    Vec3 fromCorner = toCorner.subtract(delta);
    int iterations = 0;
    for (int bx = (int) Math.floor(fromCorner.x); bx <= (int) Math.floor(toCorner.x); bx++) {
      Optional<Vec3> hit = AABB.clip(bx, 0, 0, bx + 1, 1000, 1, fromCorner, toCorner);
      if (!hit.isEmpty()) {
        iterations++;
        Vec3 h = hit.get();
        int cur = iterations;
        for (BlockPos pos : BlockPos.betweenCornersInDirection(bx, (int) Math.floor(h.y), (int) Math.floor(h.z), bx + 1, (int) Math.floor(h.y) + 1, (int) Math.floor(h.z), delta)) {
          if (visited.add(pos.asLong()) && !visitor.visit(pos, cur)) return -1;
        }
      }
    }
    return iterations;
  }
  static long total;
  static int check(Vec3 from, Vec3 to) {
    AABB box = new AABB(to.x - 0.3, to.y, to.z - 0.3, to.x + 0.3, to.y + 1.8, to.z + 0.3);
    int[] seen = {0};
    forEachBlockIntersectedBetween(from, to, box, (pos, i) -> { seen[0]++; total += pos.asLong() % 1000 + i; return true; });
    return seen[0];
  }
  public static void main(String[] args) {
    int n = 0;
    for (int i = 0; i < 300; i++) {
      n += check(new Vec3(i * 0.1, 64, 2), new Vec3(i * 0.1 + 0.4 * (i % 3), 64.2, 2.3));
    }
    System.out.println(n + " " + total);
    Iterable<BlockPos> any = BlockPos.betweenClosed(0, 0, 0, 1, 1, 1);
    Object o = any; System.out.println(o != null);
  }
}
