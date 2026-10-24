package shapes;

import java.util.ArrayList;
import java.util.List;
import shapes.round.Circle;

// `Class.getPackage()`: one `Package` per package name, null for arrays and primitive types, and
// the JDK's packages, which have no versioning information. (The manifest attributes of the
// program's own packages depend on the jar it is run from, which the translation does not know.)
public class Main {
    static String describe(Class<?> c) {
        Package p = c.getPackage();
        if (p == null) {
            return c.getName() + ": no package";
        }
        return c.getName() + ": [" + p.getName() + "] " + p.hashCode();
    }

    public static void main(String[] args) {
        Package mine = Main.class.getPackage();
        System.out.println(mine.getName() + " " + mine.hashCode());
        System.out.println(mine.getName().replace('.', '/'));
        System.out.println(mine == new Main().getClass().getPackage());
        System.out.println(mine.equals(Main.class.getPackage()));
        Package round = Circle.class.getPackage();
        System.out.println(round.getName() + " " + (round == mine) + " " + round.equals(mine));
        System.out.println(round == new Circle(2).getClass().getPackage());

        System.out.println(int.class.getPackage() == null);
        System.out.println(void.class.getPackage() == null);
        System.out.println(int[].class.getPackage() == null);
        System.out.println(Circle[].class.getPackage() == null);
        System.out.println(Circle[].class.getPackageName());

        Package lang = String.class.getPackage();
        System.out.println(lang);
        System.out.println(lang.getName() + " " + lang.hashCode() + " " + lang.isSealed());
        System.out.println(lang.getSpecificationTitle() + " " + lang.getSpecificationVersion() + " "
                + lang.getSpecificationVendor());
        System.out.println(lang.getImplementationTitle() + " " + lang.getImplementationVersion() + " "
                + lang.getImplementationVendor());
        System.out.println(lang == Integer.class.getPackage());
        System.out.println(ArrayList.class.getPackage() == List.class.getPackage());
        System.out.println(ArrayList.class.getPackage() == lang);
        System.out.println("" + List.class.getPackage());

        Object[] values = {"text", 1, new Main(), new Circle(1), new int[0], new ArrayList<String>()};
        for (Object value : values) {
            System.out.println(describe(value.getClass()));
        }
    }
}
