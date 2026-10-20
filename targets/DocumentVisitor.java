// Features: the visitor pattern with a generic interface Visitor<R> and a generic method
// <R> R accept(Visitor<R>), several visitor implementations (rendering, counting, outline with an
// indent field), double dispatch through overloaded visit methods, a sealed-style hierarchy of
// nodes with a List of children, an anonymous Visitor class, and StringBuilder output.
import java.util.ArrayList;
import java.util.List;

public class DocumentVisitor {
    interface Visitor<R> {
        R visitText(Text text);

        R visitSection(Section section);

        R visitImage(Image image);
    }

    static abstract class Node {
        abstract <R> R accept(Visitor<R> visitor);
    }

    static class Text extends Node {
        final String content;

        Text(String content) {
            this.content = content;
        }

        <R> R accept(Visitor<R> visitor) {
            return visitor.visitText(this);
        }
    }

    static class Image extends Node {
        final String file;
        final int width;

        Image(String file, int width) {
            this.file = file;
            this.width = width;
        }

        <R> R accept(Visitor<R> visitor) {
            return visitor.visitImage(this);
        }
    }

    static class Section extends Node {
        final String title;
        final List<Node> children = new ArrayList<>();

        Section(String title, Node... nodes) {
            this.title = title;
            for (Node n : nodes) {
                children.add(n);
            }
        }

        <R> R accept(Visitor<R> visitor) {
            return visitor.visitSection(this);
        }
    }

    static class Renderer implements Visitor<String> {
        public String visitText(Text text) {
            return text.content;
        }

        public String visitImage(Image image) {
            return "[image " + image.file + " @" + image.width + "]";
        }

        public String visitSection(Section section) {
            StringBuilder sb = new StringBuilder("== " + section.title + " ==\n");
            for (Node child : section.children) {
                sb.append(child.accept(this)).append('\n');
            }
            return sb.toString();
        }
    }

    static class Counter implements Visitor<Integer> {
        public Integer visitText(Text text) {
            return text.content.split(" ").length;
        }

        public Integer visitImage(Image image) {
            return 0;
        }

        public Integer visitSection(Section section) {
            int words = 0;
            for (Node child : section.children) {
                words += child.accept(this);
            }
            return words;
        }
    }

    static class Outline implements Visitor<Void> {
        private int depth = 0;
        private final StringBuilder out = new StringBuilder();

        public Void visitText(Text text) {
            return null;
        }

        public Void visitImage(Image image) {
            out.append("  ".repeat(depth)).append("image:").append(image.file).append('\n');
            return null;
        }

        public Void visitSection(Section section) {
            out.append("  ".repeat(depth)).append(section.title).append('\n');
            depth++;
            for (Node child : section.children) {
                child.accept(this);
            }
            depth--;
            return null;
        }

        String result() {
            return out.toString();
        }
    }

    public static void main(String[] args) {
        Node doc = new Section("Report",
                new Text("Quarterly numbers look good"),
                new Section("Details",
                        new Text("Revenue grew by ten percent"),
                        new Image("chart.png", 640),
                        new Section("Appendix", new Text("raw data attached"), new Image("table.png", 320))),
                new Text("Thanks for reading"));

        System.out.print(doc.accept(new Renderer()));

        int words = doc.accept(new Counter());
        System.out.println("words: " + words);

        Outline outline = new Outline();
        doc.accept(outline);
        System.out.print(outline.result());

        int images = doc.accept(new Visitor<Integer>() {
            public Integer visitText(Text text) {
                return 0;
            }

            public Integer visitImage(Image image) {
                return 1 + (image.width > 400 ? 1 : 0);
            }

            public Integer visitSection(Section section) {
                int total = 0;
                for (Node child : section.children) {
                    total += child.accept(this);
                }
                return total;
            }
        });
        System.out.println("image score: " + images);

        List<Node> flat = new ArrayList<>();
        flatten(doc, flat);
        System.out.println("nodes: " + flat.size());
    }

    static void flatten(Node node, List<Node> out) {
        out.add(node);
        if (node instanceof Section s) {
            for (Node child : s.children) {
                flatten(child, out);
            }
        }
    }
}
