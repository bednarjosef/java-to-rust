// A small text adventure driven by a state machine. Commands are strings, states are
// an enum with fields and a constructor, and transitions switch on the state's ordinal.
public class AdventureStateMachine {
    enum State {
        HALL("a dusty hall", 0),
        LIBRARY("a library of torn books", 1),
        VAULT("a locked vault", 2),
        EXIT("the open air", 3);

        final String description;
        final int depth;

        State(String description, int depth) {
            this.description = description;
            this.depth = depth;
        }

        boolean isDeeper(State other) {
            return depth > other.depth;
        }
    }

    static class Player {
        State at = State.HALL;
        boolean hasKey;
        int moves;
        int score;
        String[] items;
        int itemCount;
        State furthest = State.HALL;

        Player() {
            items = new String[4];
        }

        void take(String item) {
            if (itemCount == items.length) {
                System.out.println("pack is full");
                return;
            }
            items[itemCount] = item;
            itemCount++;
            score += item.length();
        }

        boolean has(String item) {
            for (int i = 0; i < itemCount; i++) {
                if (items[i].equals(item)) return true;
            }
            return false;
        }

        void moveTo(State next) {
            at = next;
            moves++;
            if (next.isDeeper(furthest)) furthest = next;
        }
    }

    // Returns the state reached, or the same state when the command is refused.
    static State step(Player p, String command) {
        int here = p.at.ordinal();
        switch (here) {
            case 0:
                if (command.equals("north")) {
                    p.moveTo(State.LIBRARY);
                    return State.LIBRARY;
                }
                if (command.equals("take key")) {
                    p.hasKey = true;
                    p.take("key");
                }
                return p.at;
            case 1:
                if (command.equals("south")) {
                    p.moveTo(State.HALL);
                    return State.HALL;
                }
                if (command.equals("take book")) {
                    p.take("book");
                    return p.at;
                }
                if (command.equals("east") && p.hasKey) {
                    p.moveTo(State.VAULT);
                    return State.VAULT;
                }
                return p.at;
            case 2:
                if (command.equals("west")) {
                    p.moveTo(State.LIBRARY);
                    return State.LIBRARY;
                }
                if (command.equals("take gold") && p.has("key")) {
                    p.take("gold");
                    p.moveTo(State.EXIT);
                    return State.EXIT;
                }
                return p.at;
            default:
                return p.at;
        }
    }

    static String verdict(Player p) {
        return switch (p.at.ordinal()) {
            case 3 -> "escaped";
            default -> "trapped";
        };
    }

    static void play(String[] commands) {
        Player p = new Player();
        for (String command : commands) {
            State before = p.at;
            State after = step(p, command);
            String note = before == after ? "still " + after.description : "now in " + after.description;
            System.out.println("> " + command + ": " + note);
        }
        System.out.println(verdict(p) + " after " + p.moves + " moves, score " + p.score);
        System.out.print("items:");
        for (int i = 0; i < p.itemCount; i++) System.out.print(" " + p.items[i]);
        System.out.println(" furthest " + p.furthest.depth);
    }

    public static void main(String[] args) {
        play(new String[] { "take key", "north", "take book", "east", "south", "east", "north", "east", "take gold" });
        play(new String[] { "north", "east", "take gold", "dance" });
        play(new String[] { "take key", "north", "east", "take gold", "west", "take gold" });
        for (State s : State.values()) {
            System.out.println(s.ordinal() + " " + s.depth + " " + s.isDeeper(State.LIBRARY) + " " + s);
        }
    }
}
