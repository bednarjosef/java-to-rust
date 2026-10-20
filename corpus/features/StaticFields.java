class Ids {
    static int next = 1;
    static final int FIRST = 100;
    static String label = "id";
    static double scale = 0.5;
    static boolean enabled;

    static int take() {
        int id = next;
        next += 1;
        return id;
    }
}

class Tally {
    static int count;
    int own;

    Tally(int own) {
        this.own = own;
        count++;
    }
}

public class StaticFields {
    public static void main(String[] args) {
        System.out.println(Ids.take());
        System.out.println(Ids.take());
        System.out.println(Ids.next);
        System.out.println(Ids.FIRST + Ids.next);
        Ids.label += "-x";
        System.out.println(Ids.label);
        Ids.scale *= 4;
        System.out.println(Ids.scale);
        System.out.println(Ids.enabled);
        Ids.enabled = !Ids.enabled;
        System.out.println(Ids.enabled);
        Tally a = new Tally(3);
        Tally b = new Tally(4);
        System.out.println(Tally.count + a.own + b.own);
        Tally.count = 0;
        Tally.count++;
        System.out.println(Tally.count);
    }
}
