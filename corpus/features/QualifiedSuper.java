public class QualifiedSuper {
    static class Shape {
        String name() {
            return "shape";
        }
    }

    static class Circle extends Shape {
        String name() {
            return "circle";
        }

        class Label {
            String text() {
                return Circle.super.name() + " in " + name();
            }
        }
    }

    String name;

    QualifiedSuper(String name) {
        this.name = name;
    }

    class Part {
        int size;

        Part(int size) {
            this.size = size;
        }

        String describe() {
            return name + ":" + size;
        }
    }

    static class Derived extends Part {
        Derived(QualifiedSuper owner, int n) {
            owner.super(n * 2);
        }
    }

    class Twin extends Part {
        Twin(QualifiedSuper other) {
            other.super(7);
        }
    }

    public static void main(String[] args) {
        QualifiedSuper a = new QualifiedSuper("alpha");
        QualifiedSuper b = new QualifiedSuper("beta");
        Derived d = new Derived(b, 4);
        System.out.println(d.describe());
        System.out.println(a.new Twin(b).describe());
        System.out.println(a.new Part(3).describe());
        System.out.println(new Circle().new Label().text());
    }
}
