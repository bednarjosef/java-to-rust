// Features: Scanner on System.in reading an int count, then nextLine for names, nextInt and
// nextDouble for scores, hasNextInt to skip bad tokens, a Student class with a grade method, an
// int[] histogram, Math.round and String.format with %.2f, a TreeMap of grade letters to names,
// a bad-input exception handled with try/catch, and a summary with averages and the best student.
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Scanner;
import java.util.TreeMap;

public class ScannerGrades {
    static class Student {
        final String name;
        final double[] scores;

        Student(String name, double[] scores) {
            this.name = name;
            this.scores = scores;
        }

        double average() {
            double sum = 0;
            for (double s : scores) {
                sum += s;
            }
            return scores.length == 0 ? 0 : sum / scores.length;
        }

        char letter() {
            double avg = average();
            if (avg >= 90) {
                return 'A';
            } else if (avg >= 80) {
                return 'B';
            } else if (avg >= 70) {
                return 'C';
            } else if (avg >= 60) {
                return 'D';
            }
            return 'F';
        }
    }

    static class MalformedRecordException extends Exception {
        MalformedRecordException(String message) {
            super(message);
        }
    }

    static Student readStudent(Scanner in, int line) throws MalformedRecordException {
        if (!in.hasNextLine()) {
            throw new MalformedRecordException("record " + line + " missing");
        }
        String name = in.nextLine().trim();
        if (name.isEmpty()) {
            throw new MalformedRecordException("record " + line + " has no name");
        }
        int count = in.nextInt();
        if (count < 0) {
            throw new MalformedRecordException("record " + line + " has negative count " + count);
        }
        double[] scores = new double[count];
        for (int i = 0; i < count; i++) {
            scores[i] = in.nextDouble();
        }
        in.nextLine();
        return new Student(name, scores);
    }

    public static void main(String[] args) {
        Scanner in = new Scanner(System.in);
        int declared = in.nextInt();
        in.nextLine();
        List<Student> students = new ArrayList<>();
        int rejected = 0;
        for (int i = 1; i <= declared; i++) {
            try {
                students.add(readStudent(in, i));
            } catch (MalformedRecordException e) {
                rejected++;
                System.out.println("skipped: " + e.getMessage());
                if (in.hasNextLine()) {
                    in.nextLine();
                }
            } catch (java.util.InputMismatchException e) {
                rejected++;
                System.out.println("skipped record " + i + ": bad number");
                in.nextLine();
            }
        }
        int trailing = 0;
        while (in.hasNext()) {
            if (in.hasNextInt()) {
                trailing += in.nextInt();
            } else {
                in.next();
            }
        }
        in.close();

        int[] histogram = new int[6];
        Map<Character, List<String>> byLetter = new TreeMap<>();
        Student best = null;
        for (Student s : students) {
            System.out.println(String.format("%-8s avg %6.2f grade %c", s.name, s.average(), s.letter()));
            histogram[s.letter() - 'A']++;
            byLetter.computeIfAbsent(s.letter(), k -> new ArrayList<>()).add(s.name);
            if (best == null || s.average() > best.average()) {
                best = s;
            }
        }
        System.out.println("accepted " + students.size() + ", rejected " + rejected);
        System.out.println("trailing numbers total " + trailing);
        if (best != null) {
            System.out.println("best: " + best.name + " with " + Math.round(best.average()));
        }
        for (Map.Entry<Character, List<String>> e : byLetter.entrySet()) {
            System.out.println(e.getKey() + ": " + e.getValue());
        }
        System.out.println("histogram A..F: " + Arrays.toString(histogram));
    }
}
