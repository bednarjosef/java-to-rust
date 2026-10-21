// A word index over a few lines of text: an open-addressing table of words, each with the lines
// it occurs on. Its inner classes `Entry` and `Cursor` read the index's tables directly.
public class TextIndex {
    private String[] words = new String[8];
    private int[][] lines = new int[8][];
    private int[] counts = new int[8];
    private int size;
    private int probes;

    private int slot(String word) {
        int mask = words.length - 1;
        int i = word.hashCode() & mask;
        while (words[i] != null && !words[i].equals(word)) {
            probes++;
            i = (i + 1) & mask;
        }
        return i;
    }

    private void grow() {
        String[] oldWords = words;
        int[][] oldLines = lines;
        int[] oldCounts = counts;
        words = new String[oldWords.length * 2];
        lines = new int[oldWords.length * 2][];
        counts = new int[oldWords.length * 2];
        for (int i = 0; i < oldWords.length; i++) {
            if (oldWords[i] != null) {
                int s = slot(oldWords[i]);
                words[s] = oldWords[i];
                lines[s] = oldLines[i];
                counts[s] = oldCounts[i];
            }
        }
    }

    void add(String word, int line) {
        if ((size + 1) * 4 > words.length * 3) grow();
        int s = slot(word);
        if (words[s] == null) {
            words[s] = word;
            lines[s] = new int[2];
            size++;
        }
        if (counts[s] > 0 && lines[s][counts[s] - 1] == line) return;
        if (counts[s] == lines[s].length) {
            int[] bigger = new int[counts[s] * 2];
            for (int i = 0; i < counts[s]; i++) bigger[i] = lines[s][i];
            lines[s] = bigger;
        }
        lines[s][counts[s]++] = line;
    }

    void addText(String text, int line) {
        StringBuilder word = new StringBuilder();
        for (int i = 0; i <= text.length(); i++) {
            char c = i < text.length() ? Character.toLowerCase(text.charAt(i)) : ' ';
            if (Character.isLetter(c)) {
                word.append(c);
            } else if (word.length() > 0) {
                add(word.toString(), line);
                word.setLength(0);
            }
        }
    }

    // A view of one word's slot in the tables.
    class Entry {
        private final int slot;
        Entry(int slot) { this.slot = slot; }
        String word() { return words[slot]; }
        int occurrences() { return counts[slot]; }
        boolean onLine(int line) {
            for (int i = 0; i < counts[slot]; i++) {
                if (lines[slot][i] == line) return true;
            }
            return false;
        }
        String describe() {
            StringBuilder sb = new StringBuilder(words[slot]).append(':');
            for (int i = 0; i < counts[slot]; i++) sb.append(' ').append(lines[slot][i]);
            return sb.toString();
        }
    }

    Entry find(String word) {
        int s = slot(word.toLowerCase());
        return words[s] == null ? null : new Entry(s);
    }

    // Walks the occupied slots in table order.
    class Cursor {
        private int at = -1;
        boolean advance() {
            do {
                at++;
            } while (at < words.length && words[at] == null);
            return at < words.length;
        }
        Entry entry() { return new Entry(at); }
    }

    Cursor cursor() { return new Cursor(); }

    public static void main(String[] args) {
        String[] text = {
            "The quick brown fox jumps over the lazy dog.",
            "A lazy afternoon: the dog sleeps, the fox waits.",
            "Quick thinking saves the day; brown bread saves lunch.",
            "Over and over, the fox returns.",
        };
        TextIndex index = new TextIndex();
        for (int i = 0; i < text.length; i++) index.addText(text[i], i + 1);
        System.out.println(index.size + " words in a table of " + index.words.length);

        for (String w : new String[] { "fox", "Lazy", "saves", "cat", "the" }) {
            Entry e = index.find(w);
            System.out.println(e == null ? w + ": none" : e.describe() + " (" + e.occurrences() + " lines)");
        }

        int onTwo = 0;
        String longest = "";
        Cursor c = index.cursor();
        while (c.advance()) {
            Entry e = c.entry();
            if (e.onLine(2)) onTwo++;
            if (e.word().length() > longest.length()
                    || e.word().length() == longest.length() && e.word().compareTo(longest) < 0) {
                longest = e.word();
            }
        }
        System.out.println(onTwo + " words on line 2; longest " + longest);
        Entry quick = index.find("QUICK");
        index.add("quick", 4);
        System.out.println(quick.describe() + " after adding line 4");
    }
}
