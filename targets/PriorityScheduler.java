// Features: PriorityQueue with a Comparator lambda and with Comparator.comparingInt, a user Task
// class implementing Comparable, poll and peek, a simulated clock with a while loop, switch on
// a String, and String.format with width and zero padding.
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.PriorityQueue;

public class PriorityScheduler {
    static class Task implements Comparable<Task> {
        private final String name;
        private final int priority;
        private final int duration;

        Task(String name, int priority, int duration) {
            this.name = name;
            this.priority = priority;
            this.duration = duration;
        }

        String getName() {
            return name;
        }

        int getPriority() {
            return priority;
        }

        int getDuration() {
            return duration;
        }

        @Override
        public int compareTo(Task other) {
            if (priority != other.priority) {
                return Integer.compare(other.priority, priority);
            }
            return name.compareTo(other.name);
        }

        @Override
        public String toString() {
            return String.format("%s(p%d,%dms)", name, priority, duration);
        }
    }

    static String describe(String kind) {
        switch (kind) {
            case "urgent":
                return "!!";
            case "normal":
                return "--";
            default:
                return "..";
        }
    }

    static List<String> runAll(PriorityQueue<Task> queue) {
        List<String> log = new ArrayList<>();
        int clock = 0;
        while (!queue.isEmpty()) {
            Task t = queue.poll();
            clock += t.getDuration();
            log.add(String.format("t=%04d %-8s done", clock, t.getName()));
        }
        return log;
    }

    public static void main(String[] args) {
        PriorityQueue<Task> natural = new PriorityQueue<>();
        natural.add(new Task("backup", 2, 30));
        natural.add(new Task("email", 5, 5));
        natural.add(new Task("report", 3, 45));
        natural.add(new Task("deploy", 5, 20));
        natural.add(new Task("cleanup", 1, 10));
        System.out.println("peek: " + natural.peek() + " size " + natural.size());
        for (String line : runAll(natural)) {
            System.out.println(line);
        }

        PriorityQueue<Task> byDuration = new PriorityQueue<>(Comparator.comparingInt(Task::getDuration));
        byDuration.add(new Task("a", 1, 50));
        byDuration.add(new Task("b", 1, 7));
        byDuration.add(new Task("c", 1, 23));
        StringBuilder order = new StringBuilder();
        while (!byDuration.isEmpty()) {
            order.append(byDuration.poll().getName());
        }
        System.out.println("by duration: " + order);

        PriorityQueue<String> words = new PriorityQueue<>(10, (x, y) -> y.length() - x.length());
        for (String w : "the longest word wins over shorter ones".split(" ")) {
            words.offer(w);
        }
        System.out.print("longest first:");
        while (words.size() > 3) {
            System.out.print(" " + words.poll());
        }
        System.out.println();
        System.out.println("left in queue: " + words.size() + " empty? " + words.isEmpty());

        PriorityQueue<Integer> minHeap = new PriorityQueue<>();
        int[] values = {42, 7, 19, 3, 88, 3, 56};
        for (int v : values) {
            minHeap.offer(v);
        }
        int smallest = minHeap.poll();
        int secondSmallest = minHeap.poll();
        System.out.println("smallest " + smallest + ", next " + secondSmallest + ", describe: " + describe("urgent"));
        System.out.println("remaining: " + minHeap.size() + " " + describe("idle") + describe("normal"));
    }
}
