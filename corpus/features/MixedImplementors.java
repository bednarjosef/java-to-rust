public class MixedImplementors {
    interface Task { String run(); }
    static class Quiet implements Task { public String run() { return "quiet"; } }
    static class Loud implements Task { int runs; public String run() { runs++; return "loud " + runs; } }
    static Task shared = new Quiet();
    public static void main(String[] args) {
        Task[] ts = { new Quiet(), new Loud() };
        for (Task t : ts) System.out.println(t.run() + t.run());
        System.out.println(shared.run());
    }
}
