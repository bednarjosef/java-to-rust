// Patterns in the conditions of `if`s as decompilers write them: a negated `&&` chain with a
// pattern whose variable the rest of the block reads, and a negated pattern in an `||` inside an
// `&&` chain whose later operand reads its variable.
public class NegatedPatternChains {
    static class Animal {
        final String owner;

        Animal(String owner) {
            this.owner = owner;
        }
    }

    static class Pet extends Animal {
        Pet(String owner) {
            super(owner);
        }
    }

    static int range(Object column, boolean empty) {
        if (!(!empty && column instanceof int[] range)) {
            return -1;
        }
        return range[1] - range[0];
    }

    static boolean sameOwner(boolean alerted, Animal mob, Animal other) {
        if (alerted && (!(mob instanceof Pet pet) || pet.owner.equals(other.owner)) && other != mob) {
            return true;
        }
        return false;
    }

    public static void main(String[] args) {
        System.out.println(range(new int[] {3, 10}, false) + " " + range(new int[] {3, 10}, true) + " " + range("x", false));
        Animal wild = new Animal("nobody");
        Pet a = new Pet("alex");
        Pet b = new Pet("alex");
        Pet c = new Pet("sam");
        System.out.println(sameOwner(true, a, b) + " " + sameOwner(true, a, c) + " " + sameOwner(true, wild, c)
            + " " + sameOwner(false, a, b) + " " + sameOwner(true, a, a));
    }
}
