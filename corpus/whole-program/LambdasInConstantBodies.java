// Lambdas and method references in the methods of enum constants' bodies, as javac's
// `Infer.IncorporationBinaryOpKind` and `Option` have: they capture the method's parameters,
// its locals and the constant itself.

import java.util.List;
import java.util.function.Predicate;
import java.util.stream.Collectors;

public class LambdasInConstantBodies {
    enum Filter {
        EVEN {
            @Override
            List<Integer> keep(List<Integer> values, int limit) {
                return values.stream().filter(v -> v % 2 == 0 && v < limit).collect(Collectors.toList());
            }
        },
        ABOVE {
            @Override
            List<Integer> keep(List<Integer> values, int floor) {
                int shift = floor * 2;
                Predicate<Integer> high = v -> v + shift > floor * 3;
                return values.stream().filter(high).collect(Collectors.toList());
            }
        },
        NAMED("named") {
            @Override
            List<Integer> keep(List<Integer> other, int limit) {
                return other.stream().filter(v -> label.length() + v > limit).map(this::twice).collect(Collectors.toList());
            }
        },
        ALL;

        final String label;

        Filter() {
            this("plain");
        }

        Filter(String label) {
            this.label = label;
        }

        int twice(int v) {
            return v * 2;
        }

        List<Integer> keep(List<Integer> values, int limit) {
            return values;
        }
    }

    public static void main(String[] args) {
        List<Integer> values = List.of(1, 2, 3, 4, 5, 6, 7, 8);
        for (Filter f : Filter.values()) {
            System.out.println(f + " " + f.label + ": " + f.keep(values, 6));
        }
    }
}
