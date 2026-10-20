class Point {
    int x;
    int y;

    Point(int x, int y) {
        this.x = x;
        this.y = y;
    }

    public String toString() {
        return "(" + x + ", " + y + ")";
    }
}

class Account {
    int balance;

    Account(int balance) {
        this.balance = balance;
    }

    public String toString() {
        return "Account[" + balance + "]";
    }
}

public class ToString {
    static Point origin() {
        return new Point(0, 0);
    }

    public static void main(String[] args) {
        Point p = new Point(3, 4);
        System.out.println(p);
        System.out.println("at " + p + " and " + origin());
        String s = "p=";
        s += p;
        System.out.println(s);
        Account a = new Account(5);
        a.balance += 2;
        System.out.println(a + " " + a.balance);
        System.out.println(origin().toString().length());
        System.out.println(p.toString());
    }
}
