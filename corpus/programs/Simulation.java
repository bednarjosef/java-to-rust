public class Simulation {
    interface Actor {
        String name();
        int energy();
        default void tick(World w) { w.log(name() + " waits"); }
        default boolean alive() { return energy() > 0; }
    }

    static class World {
        String journal = "";
        int day;
        void log(String s) { journal += day + ":" + s + ";"; }
    }

    static class Rabbit implements Actor {
        int food = 3;
        public String name() { return "rabbit"; }
        public int energy() { return food; }
        public void tick(World w) {
            food--;
            w.log("rabbit eats, food " + food);
        }
    }

    static class Fox implements Actor {
        int hunger;
        final String tag;
        Fox(String tag) { this.tag = tag; }
        public String name() { return "fox " + tag; }
        public int energy() { return 5 - hunger; }
        public void tick(World w) {
            hunger += 2;
            if (hunger > 3) w.log(name() + " is hungry");
            else Actor.super.tick(w);
        }
    }

    enum Season implements Actor {
        SPRING(4), WINTER(0);
        final int warmth;
        Season(int warmth) { this.warmth = warmth; }
        public int energy() { return warmth; }
    }

    public static void main(String[] args) {
        World w = new World();
        Actor[] actors = { new Rabbit(), new Fox("red"), Season.SPRING, new Fox("grey"), Season.WINTER };
        for (int day = 1; day <= 3; day++) {
            w.day = day;
            for (Actor a : actors) {
                if (a.alive()) a.tick(w);
            }
        }
        System.out.println(w.journal);
        int alive = 0;
        for (Actor a : actors) if (a.alive()) alive++;
        System.out.println(alive + " alive");
        Actor first = actors[2];
        System.out.println(first.name() + " " + (first == Season.SPRING) + " " + (actors[4] == first));
    }
}
