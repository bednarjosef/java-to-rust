// Variables of one name declared in sibling scopes are different variables: an object that one
// of them holds within a loop's iteration can live on the stack although another variable of the
// same name, declared outside the loop, holds objects that escape. A name used for a pattern
// variable stays one variable, as its scope follows the flow of the code.
public class StackObjectsOfSameNamedVariables {
    static class Point {
        int x;
        int y;
        Point next;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }
    }

    public static void main(String[] args) {
        Point keep = null;
        for (Point p = new Point(0, 0); p.x < 3; p.x++) {
            keep = p;
        }
        long sum = 0;
        for (int i = 0; i < 5; i++) {
            Point p = new Point(i, i * 2);
            sum += p.x + p.y;
        }
        Point last = null;
        for (int i = 0; i < 3; i++) {
            Point p = new Point(i, 10 * i);
            p.next = last;
            last = p;
        }
        for (Point p = last; p != null; p = p.next) {
            sum = sum * 31 + p.y;
        }
        {
            Point q = new Point(7, 8);
            sum += q.x * q.y;
        }
        {
            Point q = keep;
            q.y = 9;
        }
        Object o = keep;
        for (int i = 0; i < 2; i++) {
            Point pt = new Point(i, 1);
            if (o instanceof Point pt2 && pt2.y == 9) {
                sum += pt.x + pt2.x;
            }
        }
        if (o instanceof Point pt) {
            sum += pt.y;
        }
        System.out.println(sum + " " + keep.x + " " + keep.y + " " + last.x + " " + last.next.y);
    }
}
