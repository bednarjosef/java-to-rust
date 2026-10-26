package com.example.raid;

import com.example.monster.Patrolling;

// The messages of NullPointerExceptions of a nested class's uses of the protected members of a
// superclass in another package, which javac makes through accessor methods of the subclass
// (`access$000`): they dereference the accessor's parameter, and a protected static method
// dereferences its own.
public class Main extends Patrolling {
    static class Goal {
        private final Main mob;

        Goal(Main mob) {
            this.mob = mob;
        }

        boolean canUse() {
            return this.mob.isPatrolling() && this.mob.patrolling;
        }

        boolean flag() {
            return this.mob.patrolling;
        }

        void stop() {
            this.mob.patrolling = false;
        }

        int round() {
            return ++this.mob.rounds;
        }

        int counted() {
            return count(this.mob);
        }
    }

    interface Action {
        void run();
    }

    static void report(Action action) {
        try {
            action.run();
            System.out.println("no exception");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        Main m = new Main();
        Goal g = new Goal(m);
        System.out.println(g.canUse() + " " + g.round() + " " + g.counted());
        g.stop();
        System.out.println(g.canUse() + " " + g.flag());
        Goal none = new Goal(null);
        report(() -> none.canUse());
        report(() -> none.flag());
        report(() -> none.stop());
        report(() -> none.round());
        report(() -> System.out.println(none.counted()));
    }
}
