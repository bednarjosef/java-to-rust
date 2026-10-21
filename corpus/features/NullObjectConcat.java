public class NullObjectConcat {
    static class Node { int val; Node(int v) { val = v; } }
    public static void main(String[] args) {
        Node[] slots = new Node[2];
        System.out.println("slot " + slots[0]);
    }
}
