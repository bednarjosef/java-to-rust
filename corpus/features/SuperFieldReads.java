// A field read through `super`, as javac's Flow reads `super.alive` in its analyzers.
public class SuperFieldReads {
    static class Analyzer {
        int alive = 1;
    }

    static class SnippetAnalyzer extends Analyzer {
        boolean isAlive() {
            return super.alive != 0;
        }

        void kill() {
            super.alive = 0;
        }
    }

    public static void main(String[] args) {
        SnippetAnalyzer a = new SnippetAnalyzer();
        System.out.println(a.isAlive());
        a.kill();
        System.out.println(a.isAlive() + " " + a.alive);
    }
}
