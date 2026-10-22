public class ParameterNullMessages {
    static class Node {
        int v;
        Node next;

        Node(int v, Node next) {
            this.v = v;
            this.next = next;
        }
    }

    interface Case {
        int run(Node n) throws Exception;
    }

    // The write comes after the use, but a later iteration reaches the use again.
    static int loopBack(Node n) {
        int c = 0;
        while (true) {
            c += n.v;
            n = n.next;
        }
    }

    // After the loop: the loop exits from its condition, before the body's write.
    static int afterLoop(Node n) {
        int k = 1;
        while (k-- > 0) {
            n = n.next;
        }
        return n.v;
    }

    static int afterLoopBreak(Node n) {
        while (true) {
            n = n.next;
            if (n == null) {
                break;
            }
        }
        return n.v;
    }

    static int afterIf(Node n) {
        if (n.v > 0) {
            n = n.next;
        }
        return n.v;
    }

    static int elseBranch(Node n) {
        if (n.v > 5) {
            n = n.next;
            return 0;
        } else {
            return n.next.next.v;
        }
    }

    // A handler starts with no slot written.
    static int inCatch(Node n) {
        n = n.next;
        try {
            if (n == null) {
                throw new IllegalStateException();
            }
            return 0;
        } catch (IllegalStateException e) {
            return n.v;
        }
    }

    static int afterCatch(Node n) {
        n = n.next;
        try {
            if (n == null) {
                throw new IllegalStateException();
            }
        } catch (IllegalStateException e) {
            System.out.print("");
        }
        return n.v;
    }

    static int forStep(Node n) {
        int s = 0;
        for (int i = 0; i < 5; n = n.next, i++) {
            s += n.v;
        }
        return s;
    }

    static int forStepContinue(Node n) {
        int s = 0;
        for (int i = 0; i < 5; i++) {
            if (i == 2) {
                n = n.next;
                continue;
            }
            s += i;
        }
        return s + n.v;
    }

    static int doWhile(Node n) {
        int s = 0;
        do {
            s += n.v;
            n = n.next;
        } while (true);
    }

    static int ternary(Node n) {
        return n.v > 0 ? (n = n.next).v : n.next.v;
    }

    static int switchFall(Node n) {
        switch (n.v) {
            case 1:
                n = n.next;
            case 2:
                return n.v;
            default:
                return n.next.v;
        }
    }

    static int switchNoFall(Node n) {
        switch (n.v) {
            case 1:
                n = n.next;
                break;
            case 2:
                return n.next.v;
        }
        return n.next.v;
    }

    // Both copies of the block read the parameter before any write.
    static int inFinally(Node n) {
        try {
            System.out.print("");
        } finally {
            System.out.print(n.v > 100 ? "" : "");
        }
        n = null;
        return 0;
    }

    static int after(Node n) {
        int k = n.v;
        n = n.next;
        return k + n.v;
    }

    static int beforeWrite(Node n) {
        int v = n.v;
        n = new Node(0, null);
        return v;
    }

    static void run(String name, Case c, Node n) {
        try {
            System.out.println(name + " " + c.run(n));
        } catch (Exception e) {
            System.out.println(name + " " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        Node one = new Node(1, null);
        Node two = new Node(2, one);
        run("loopBack", ParameterNullMessages::loopBack, two);
        run("afterLoop", ParameterNullMessages::afterLoop, one);
        run("afterLoopBreak", ParameterNullMessages::afterLoopBreak, two);
        run("afterIf", ParameterNullMessages::afterIf, one);
        run("elseBranch", ParameterNullMessages::elseBranch, one);
        run("inCatch", ParameterNullMessages::inCatch, one);
        run("afterCatch", ParameterNullMessages::afterCatch, one);
        run("forStep", ParameterNullMessages::forStep, two);
        run("forStepContinue", ParameterNullMessages::forStepContinue, one);
        run("doWhile", ParameterNullMessages::doWhile, two);
        run("ternary", ParameterNullMessages::ternary, one);
        run("switchFall", ParameterNullMessages::switchFall, one);
        run("switchNoFall", ParameterNullMessages::switchNoFall, new Node(3, null));
        run("inFinally", ParameterNullMessages::inFinally, null);
        run("after", ParameterNullMessages::after, one);
        run("beforeWrite", ParameterNullMessages::beforeWrite, null);
    }
}
