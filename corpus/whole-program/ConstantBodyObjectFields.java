import java.util.List;

// Enum constants whose bodies declare fields of object types, set by calls, as the Minecraft
// server's `SupportType.CENTER { private final VoxelShape CENTER_SUPPORT_SHAPE = Block.column(..); }`.
public class ConstantBodyObjectFields {
    static class Shape {
        final int width;

        Shape(int width) {
            this.width = width;
        }

        static Shape column(int width) {
            return new Shape(width);
        }

        boolean covers(int x) {
            return x < width;
        }
    }

    enum Support {
        FULL {
            @Override
            boolean supports(int x) {
                return true;
            }
        },
        CENTER {
            private final Shape shape = Shape.column(2);

            @Override
            boolean supports(int x) {
                return shape.covers(x);
            }
        },
        RIGID {
            private final Shape shape = Shape.column(12);
            private final List<String> names = List.of("rigid", "solid");

            @Override
            boolean supports(int x) {
                return shape.covers(x) && names.size() == 2;
            }
        };

        abstract boolean supports(int x);
    }

    public static void main(String[] args) {
        for (Support s : Support.values()) {
            System.out.println(s + " " + s.supports(1) + " " + s.supports(5) + " " + s.supports(20));
        }
    }
}
