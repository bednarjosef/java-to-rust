// Features: Scanner reading standard input line by line (TextAdventure.input), a Room class with
// exits in a HashMap<String, String>, an Item and an inventory in a List, a command dispatcher with
// a switch on the first word, a while loop until quit, a checked helper that throws, String
// methods split, trim, toLowerCase, startsWith and equals, a StringBuilder for the room
// description, and a final score line.
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Scanner;

public class TextAdventure {
    static class Room {
        final String name;
        final String description;
        final Map<String, String> exits = new HashMap<>();
        final List<String> items = new ArrayList<>();

        Room(String name, String description) {
            this.name = name;
            this.description = description;
        }

        String describe() {
            StringBuilder sb = new StringBuilder();
            sb.append(name).append(": ").append(description);
            if (!items.isEmpty()) {
                sb.append(" You see: ").append(String.join(", ", items)).append(".");
            }
            sb.append(" Exits: ").append(exits.keySet().stream().sorted().toList());
            return sb.toString();
        }
    }

    static class Game {
        private final Map<String, Room> rooms = new HashMap<>();
        private final List<String> inventory = new ArrayList<>();
        private Room current;
        private int score;
        private boolean finished;

        void addRoom(Room room) {
            rooms.put(room.name, room);
        }

        void connect(String from, String direction, String to) {
            rooms.get(from).exits.put(direction, to);
        }

        void start(String name) {
            current = rooms.get(name);
            System.out.println(current.describe());
        }

        void take(String item) {
            if (current.items.remove(item)) {
                inventory.add(item);
                score += 5;
                System.out.println("Taken: " + item);
            } else {
                System.out.println("There is no " + item + " here.");
            }
        }

        void move(String direction) {
            String target = current.exits.get(direction);
            if (target == null) {
                System.out.println("You cannot go " + direction + ".");
                return;
            }
            if (target.equals("vault") && !inventory.contains("key")) {
                System.out.println("The vault door is locked.");
                return;
            }
            current = rooms.get(target);
            if (current.name.equals("vault")) {
                score += 20;
            }
            System.out.println(current.describe());
        }

        boolean handle(String line) {
            String[] parts = line.trim().toLowerCase().split("\\s+", 2);
            String verb = parts[0];
            String rest = parts.length > 1 ? parts[1] : "";
            switch (verb) {
                case "":
                    break;
                case "go":
                    move(rest);
                    break;
                case "take":
                    take(rest);
                    break;
                case "inventory", "inv":
                    System.out.println("You carry: " + (inventory.isEmpty() ? "nothing" : String.join(", ", inventory)));
                    break;
                case "look":
                    System.out.println(current.describe());
                    break;
                case "score":
                    System.out.println("Score: " + score);
                    break;
                case "quit":
                    finished = true;
                    break;
                default:
                    System.out.println("I do not understand '" + verb + "'.");
            }
            return !finished;
        }

        int score() {
            return score;
        }
    }

    public static void main(String[] args) {
        Game game = new Game();
        Room hall = new Room("hall", "a dusty hall with a faded rug.");
        Room library = new Room("library", "shelves reach the ceiling.");
        Room vault = new Room("vault", "a cold room full of gold.");
        library.items.add("key");
        library.items.add("book");
        hall.items.add("lamp");
        game.addRoom(hall);
        game.addRoom(library);
        game.addRoom(vault);
        game.connect("hall", "north", "library");
        game.connect("library", "south", "hall");
        game.connect("library", "east", "vault");
        game.connect("vault", "west", "library");
        game.start("hall");

        Scanner in = new Scanner(System.in);
        int turns = 0;
        boolean running = true;
        while (running && in.hasNextLine()) {
            String line = in.nextLine();
            turns++;
            System.out.println("> " + line.trim());
            running = game.handle(line);
        }
        in.close();
        System.out.println("Turns taken: " + turns);
        System.out.println("Final score: " + game.score());
    }
}
