// A concatenation that calls a method changing `this` copies the fields of `this` it shows as
// it reaches them: Java reads each before the next operand runs, and the copy ends the borrow.
public class ConcatenationChangesThis {
    interface Counter {
        int next();
        default int twice() {
            return next() * 2;
        }
    }
    static class Base implements Counter {
        int count = 1;
        double weight = 0.5;
        char mark = 'a';
        public int next() {
            return count;
        }
        int peek() {
            return next() + 1;
        }
    }
    static class Ticker extends Base {
        @Override
        public int next() {
            count += 3;
            return count;
        }
        String show() {
            return "T." + count + ":" + next() + ":" + count + "/" + weight + mark + bump() + mark;
        }
        String bump() {
            weight *= 2;
            mark++;
            return "b" + weight;
        }
    }
    static class Plain extends Base {
        int read() {
            return twice() + count;
        }
    }

    public static void main(String[] args) {
        Base t = new Ticker();
        Plain p = new Plain();
        Ticker k = new Ticker();
        System.out.println(t.next() + " " + t.twice() + " " + p.read() + " " + k.peek() + " " + p.peek());
        System.out.println(k.show());
        System.out.println(k.show() + " " + k.count);
    }
}
