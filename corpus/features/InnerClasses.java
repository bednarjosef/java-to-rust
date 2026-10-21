public class InnerClasses {
    static class Account {
        String owner;
        int balance;
        int changes;

        Account(String owner, int balance) {
            this.owner = owner;
            this.balance = balance;
        }

        // An inner class that reads and writes the fields of its outer object.
        class Transfer {
            final int amount;
            Transfer(int amount) { this.amount = amount; }
            boolean apply() {
                if (balance < amount) return false;
                balance -= amount;
                changes++;
                return true;
            }
            String describe() { return owner + " pays " + amount; }
        }

        Transfer pay(int amount) { return new Transfer(amount); }

        // Two inner classes of one outer class calling each other through it.
        class Audit {
            int checks;
            String check() {
                checks++;
                return owner + ": " + balance + " after " + changes + " changes, " + summary().line();
            }
        }

        class Summary {
            String line() { return "audited " + audit().checks + " times"; }
        }

        // Created when first used, as the JDK's collections create their views.
        Audit audit;
        Summary summary;

        Audit audit() {
            if (audit == null) audit = new Audit();
            return audit;
        }

        Summary summary() {
            if (summary == null) summary = new Summary();
            return summary;
        }
    }

    static class Grid {
        int width;
        int height;
        int marks;

        Grid(int width, int height) {
            this.width = width;
            this.height = height;
        }

        // An inner class with constructors calling each other, and names that shadow the outer
        // class's.
        class Cell {
            int x;
            int y;
            int width;

            Cell(int x, int y) {
                this.x = x;
                this.y = y;
                this.width = 1;
            }

            Cell(int index) { this(index % Grid.this.width, index / Grid.this.width); }

            int index() { return y * Grid.this.width + x; }

            boolean inside() { return x + width <= Grid.this.width && y < height; }

            void mark() { marks += width; }

            String show() { return "(" + x + "," + y + ") of " + width + " in " + Grid.this.width + "x" + height; }
        }
    }

    public static void main(String[] args) {
        Account a = new Account("ann", 100);
        Account.Transfer t1 = a.pay(30);
        Account.Transfer t2 = a.new Transfer(90);
        System.out.println(t1.describe() + ": " + t1.apply());
        System.out.println(t2.describe() + ": " + t2.apply());
        System.out.println(a.audit().check());
        System.out.println(a.audit().check());
        Account b = new Account("bob", 5);
        Account.Transfer t3 = b.new Transfer(5);
        System.out.println(t3.describe() + ": " + t3.apply() + " " + b.balance + " " + a.balance);

        Grid g = new Grid(4, 3);
        Grid.Cell c = g.new Cell(6);
        System.out.println(c.show() + " index " + c.index() + " inside " + c.inside());
        Grid.Cell d = g.new Cell(3, 5);
        System.out.println(d.show() + " inside " + d.inside());
        c.mark();
        d.mark();
        c.width = 3;
        c.mark();
        System.out.println("marks " + g.marks);
    }
}
