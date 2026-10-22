public class ConcatOfThrowingOperands {
    static class Ref {
        String path;
        String spec;

        Ref(String path, String spec) {
            this.path = path;
            this.spec = spec;
        }

        String getSpec() {
            return spec;
        }

        String resolve() {
            int ind = path.lastIndexOf('/');
            if (ind == -1) {
                return "/".concat(getSpec());
            }
            return path.substring(0, ind + 1).concat(getSpec());
        }

        String relative(String base) {
            String text = base.concat(getSpec());
            return path.concat(text);
        }
    }

    static String describe(Ref ref) {
        try {
            return ref.resolve();
        } catch (NullPointerException e) {
            return "null operand";
        }
    }

    public static void main(String[] args) {
        System.out.println(describe(new Ref("a/b/c", "d")));
        System.out.println(describe(new Ref("top", "d")));
        System.out.println(describe(new Ref(null, "d")));
        System.out.println(describe(new Ref("a/b", null)));
        Ref ref = new Ref("x/y", "z");
        System.out.println(ref.relative("p"));
        ref.path = null;
        try {
            System.out.println(ref.relative("q"));
        } catch (NullPointerException e) {
            System.out.println("relative failed");
        }
        System.out.println(ref.resolve());
    }
}
