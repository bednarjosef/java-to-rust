// A method that calls an override that changes nothing is still `&mut self` when another
// method of its override group changes the object, as the call may run that method.
public class OverrideGroupMutation {
    static class Base {
        long total = 3;
        int step() {
            total *= total;
            return (int) total;
        }
    }
    static class Quiet extends Base {
        int twice(int a0) {
            return a0 + step() * 2;
        }
        @Override
        int step() {
            return 5;
        }
    }
    static class Loud extends Base {
        @Override
        int step() {
            total += 1;
            return (int) total;
        }
    }

    public static void main(String[] args) {
        Base b = new Base();
        Quiet q = new Quiet();
        Loud l = new Loud();
        Base m = l;
        System.out.println(b.step() + " " + q.twice(1) + " " + m.step() + " " + q.twice(2));
        System.out.println(b.total + " " + q.total + " " + l.total);
    }
}
