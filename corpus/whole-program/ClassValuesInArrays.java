// `Class` values that reach lookups through arrays: a `Class[][]` written as a literal and one
// made with `new Class[2][2]`, an array filled by `System.arraycopy` and by `Arrays.fill`, a copy
// (`Arrays.copyOf`) and a `clone()`, an `Object[]` holding classes, one set by reflection
// (`Array.set`), varargs, and an array kept in a static field and read in another method. The
// translation keeps in its reflection table the fields of the classes that reach a lookup this
// way (and of `Alpha`, in an array that the array read from holds too), and leaves out those of
// `Unused`, whose class only goes into an array that no lookup reads: the program prints what
// Java prints.
import java.lang.reflect.Field;
import java.util.Arrays;

class Alpha {
    int a1;
    String a2;
}

class Beta {
    long b1;
}

class Gamma {
    double g1;
    double g2;
}

class Delta {
    char d1;
}

class Epsilon {
    byte e1;
}

class Zeta {
    short z1;
}

class Eta {
    float h1;
}

class Theta {
    boolean t1;
}

class Iota {
    int i1;
}

class Kappa {
    long k1;
}

class Unused {
    int u1;
    int u2;
}

public class ClassValuesInArrays {
    static Class<?>[] kept;

    static String fields(Class<?> c) {
        Field[] declared = c.getDeclaredFields();
        String[] names = new String[declared.length];
        for (int i = 0; i < declared.length; i++) {
            names[i] = declared[i].getName();
        }
        Arrays.sort(names);
        return c.getSimpleName() + " " + String.join(",", names);
    }

    static Class<?>[] of(Class<?>... classes) {
        return classes;
    }

    static void keep(Class<?>[] classes) {
        kept = classes;
    }

    static String keptFields() {
        return fields(kept[0]);
    }

    public static void main(String[] args) {
        Class<?>[][] groups = { { Alpha.class }, { Beta.class } };
        System.out.println(fields(groups[1][0]));
        Class<?>[][] grid = new Class<?>[2][2];
        grid[1][0] = Gamma.class;
        System.out.println(fields(grid[1][0]));
        Class<?>[] source = { Delta.class };
        Class<?>[] copied = new Class<?>[1];
        System.arraycopy(source, 0, copied, 0, 1);
        System.out.println(fields(copied[0]));
        Class<?>[] filled = new Class<?>[2];
        Arrays.fill(filled, Epsilon.class);
        System.out.println(fields(filled[1]));
        Class<?>[] grown = Arrays.copyOf(of(Zeta.class), 2);
        System.out.println(fields(grown[0]));
        Class<?>[] cloned = of(Eta.class).clone();
        System.out.println(fields(cloned[0]));
        Object[] objects = { "text", Theta.class };
        System.out.println(fields((Class<?>) objects[1]));
        Object[] reflected = new Class<?>[1];
        java.lang.reflect.Array.set(reflected, 0, Kappa.class);
        System.out.println(fields((Class<?>) reflected[0]));
        keep(of(Iota.class));
        System.out.println(keptFields());
        Class<?>[] names = { Unused.class };
        System.out.println(names[0].getSimpleName() + " " + groups.length);
    }
}
