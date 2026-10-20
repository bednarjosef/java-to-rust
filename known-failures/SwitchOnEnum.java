public class SwitchOnEnum {
    enum Op { ADD, SUB }
    public static void main(String[] args) {
        Op op = Op.SUB;
        switch (op) {
            case ADD: System.out.println("add"); break;
            case SUB: System.out.println("sub"); break;
        }
    }
}
