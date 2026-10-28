/** Text made of a value cast to a class whose objects never reach the cast. */
public class ConcatOfNeverCast {
    static final java.util.List<Manager> MANAGERS = new java.util.ArrayList<>();

    abstract static class Manager {
        final String name;

        Manager(String name) {
            this.name = name;
            MANAGERS.add(this);
        }

        @Override
        public final String toString() {
            return "manager " + nameOf(this);
        }
    }

    static final class Made extends Manager {
        Made() {
            super("made");
        }
    }

    static final class NeverMade extends Manager {
        NeverMade() {
            super("never made");
        }
    }

    static String nameOf(Manager manager) {
        return manager.name;
    }

    static class Holder<T extends Manager> {
        final T manager;

        Holder(T manager) {
            this.manager = manager;
        }

        T getManager() {
            return manager;
        }
    }

    static final class NeverHolder extends Holder<NeverMade> {
        final String description;

        NeverHolder(NeverMade manager) {
            super(manager);
            description = "{ manager=" + getManager() + " }";
        }
    }

    public static void main(String[] args) {
        Holder<Made> holder = new Holder<>(new Made());
        System.out.println("{ manager=" + holder.getManager() + " }");
        System.out.println(new NeverHolder(null).description);
        System.out.println(MANAGERS.size());
    }
}
