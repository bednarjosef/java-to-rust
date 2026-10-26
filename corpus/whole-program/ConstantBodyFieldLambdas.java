// Lambdas in the initializers of the fields of enum constants' bodies, as Minecraft's
// `UpgradeData.BlockFixers.LEAVES` has (`ThreadLocal.withInitial(() -> ...)`), and the varargs
// of the enum's constructor, which a constant with a body passes through the body's anonymous
// constructor: several arguments, one, or none.

import java.util.ArrayList;
import java.util.List;
import java.util.function.Supplier;

public class ConstantBodyFieldLambdas {
    interface Fixer {
        String fix(String s);
    }

    enum Fixers implements Fixer {
        LEAVES("a", "b") {
            private final ThreadLocal<List<String>> queue = ThreadLocal.withInitial(() -> new ArrayList<>(7));
            private final Supplier<String> name = () -> "leaves" + blocks.length;

            @Override
            public String fix(String s) {
                queue.get().add(s);
                return name.get() + ":" + s + ":" + queue.get().size();
            }
        },
        ONE("c") {
            private final Supplier<String> first = () -> blocks[0];

            @Override
            public String fix(String s) {
                return first.get() + s;
            }
        },
        DEFAULT {
            @Override
            public String fix(String s) {
                return s + blocks.length;
            }
        };

        final String[] blocks;

        Fixers(String... blocks) {
            this.blocks = blocks;
        }
    }

    public static void main(String[] args) {
        System.out.println(Fixers.LEAVES.fix("x"));
        System.out.println(Fixers.LEAVES.fix("y"));
        System.out.println(Fixers.ONE.fix("z"));
        System.out.println(Fixers.DEFAULT.fix("w"));
    }
}
