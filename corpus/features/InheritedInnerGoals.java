// Inner classes of a subclass whose superclasses are inner classes of the enclosing class's
// superclass, in a class tree whose constructor calls an overridable method: each passes its own
// enclosing instance as its superclass's, as Mob goals in the Minecraft server do.
public class InheritedInnerGoals {
    static Object[] registered = new Object[8];
    static int count;

    abstract static class Goal {
        Goal() {
            registered[count++] = this;
            setFlags();
        }

        void setFlags() {}

        abstract void start();
    }

    static class Caster {
        int power = 3;

        abstract class UseGoal extends Goal {
            int warmup;

            void start() {
                warmup = power;
                cast();
            }

            abstract void cast();
        }

        class CastingGoal extends Goal {
            CastingGoal() {
                setFlags();
            }

            void start() {
                System.out.println("casting " + power);
            }
        }
    }

    static class Evoker extends Caster {
        String name = "evoker";

        class Attack extends Caster.UseGoal {
            void cast() {
                System.out.println("attack " + name + " " + power + " " + warmup);
            }
        }

        class Casting extends Caster.CastingGoal {
            void setFlags() {
                System.out.println("flags " + name);
            }
        }
    }

    public static void main(String[] args) {
        Evoker e = new Evoker();
        e.new Attack().start();
        e.new Casting().start();
        System.out.println(count);
    }
}
