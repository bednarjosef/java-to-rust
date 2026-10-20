class Person {
    String name;
    String nick;
    Person boss;
    int reports;
    Person(String name) { this.name = name; }
    void reportTo(Person b) { boss = b; b.reports++; }
    String title() {
        if (boss == null) return name + " (top)";
        return name + " under " + boss.name;
    }
    public String toString() { return name + "/" + nick; }
}
class Registry {
    static Person first;
    static Person last;
    static int count;
    static void add(Person p) {
        if (first == null) first = p;
        last = p;
        count++;
    }
}
public class NullObjects {
    static Person pick(Person a, Person b) { return a != null ? a : b; }
    static int depth(Person p) {
        if (p == null) return 0;
        return 1 + depth(p.boss);
    }
    static String nameOf(Person p) {
        if (p == null) return "nobody";
        return p.name;
    }
    public static void main(String[] args) {
        Person ceo = new Person("Ann");
        Person cto = new Person("Bob");
        Person dev = new Person("Cid");
        cto.reportTo(ceo);
        dev.reportTo(cto);
        dev.nick = "c";
        System.out.println(ceo.title() + "; " + cto.title() + "; " + dev.title());
        System.out.println(ceo.reports + " " + cto.reports + " " + dev.reports);
        System.out.println(depth(dev) + " " + depth(null));
        System.out.println(pick(null, dev).name + " " + pick(ceo, dev).name);
        System.out.println(dev + " " + ceo);
        System.out.println(nameOf(dev.boss.boss) + " " + nameOf(ceo.boss));
        Registry.add(ceo);
        Registry.add(dev);
        System.out.println(Registry.first.name + " " + Registry.last.name + " " + Registry.count);
        Person nobody = null;
        System.out.println(nobody == Registry.first);
        System.out.println(ceo.nick == null);
        System.out.println(dev.boss.boss.boss == null ? "end" : "more");
        System.out.println(ceo.boss.name);
    }
}
