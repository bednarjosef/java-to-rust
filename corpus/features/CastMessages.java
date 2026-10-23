// The message of a ClassCastException, HotSpot's: the class of the object, the class it is cast
// to, and the module and class loader of each ("class A cannot be cast to class B (A and B are in
// unnamed module of loader 'app')"). The program's classes are in the unnamed module of the system
// class loader, strings, boxes and arrays of primitives in java.base. (Run as `java File.java`, the
// program's loader is the source launcher's, named with an identity hash, so `show` names any
// loader that is not quoted 'app'.)
public class CastMessages {
    interface Pet {
        String name();
    }

    static class Animal {
        String sound() {
            return "...";
        }
    }

    static class Dog extends Animal implements Pet {
        public String name() {
            return "dog";
        }

        String sound() {
            return "woof";
        }
    }

    static class Cat extends Animal {
        String sound() {
            return "meow";
        }
    }

    static Object pick(int i) {
        switch (i) {
            case 0:
                return new Dog();
            case 1:
                return new Cat();
            case 2:
                return Integer.valueOf(5);
            case 3:
                return "text";
            default:
                return new int[3];
        }
    }

    static Animal animal(int i) {
        return i == 0 ? new Dog() : new Cat();
    }

    static void show(ClassCastException e) {
        String m = e.getMessage();
        StringBuilder out = new StringBuilder();
        int at = 0;
        while (true) {
            int loader = m.indexOf("loader ", at);
            if (loader < 0) {
                break;
            }
            int name = loader + "loader ".length();
            out.append(m, at, name);
            if (m.charAt(name) == '\'') {
                at = name;
            } else {
                out.append("'app'");
                int end = name;
                while (end < m.length() && m.charAt(end) != ';' && m.charAt(end) != ')') {
                    end++;
                }
                at = end;
            }
        }
        out.append(m.substring(at));
        System.out.println(out);
    }

    public static void main(String[] args) {
        try {
            Cat cat = (Cat) pick(0);
            System.out.println("not reached " + cat.sound());
        } catch (ClassCastException e) {
            show(e);
        }
        try {
            Dog dog = (Dog) animal(1);
            System.out.println("not reached " + dog.sound());
        } catch (ClassCastException e) {
            show(e);
        }
        try {
            Pet pet = (Pet) pick(1);
            System.out.println("not reached " + pet.name());
        } catch (ClassCastException e) {
            show(e);
        }
        try {
            String text = (String) pick(2);
            System.out.println("not reached " + text.length());
        } catch (ClassCastException e) {
            show(e);
        }
        try {
            Dog dog = (Dog) pick(3);
            System.out.println("not reached " + dog.sound());
        } catch (ClassCastException e) {
            show(e);
        }
        try {
            String text = (String) pick(0);
            System.out.println("not reached " + text.length());
        } catch (ClassCastException e) {
            show(e);
        }
        try {
            Cat cat = (Cat) pick(4);
            System.out.println("not reached " + cat.sound());
        } catch (ClassCastException e) {
            show(e);
        }
        // Uncaught, it ends the program.
        Cat cat = (Cat) pick(0);
        System.out.println("not reached " + cat.sound());
    }
}
