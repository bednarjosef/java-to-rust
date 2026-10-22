// An enum whose constructor prints: Java builds the constants one by one, in order, when the enum
// is first used, before the rest of its static initializer, and a constant's body's field
// initializers run right after its constructor.
public class EnumConstructorOrder {
    enum Step {
        FIRST("one"),
        SECOND("two") {
            final String note = say("body field of " + name());

            @Override
            String describe() { return "second with " + note; }
        },
        THIRD("three");

        static final String AFTER = say("rest of the static initializer");
        static int built;

        final String word;

        Step(String word) {
            this.word = say("constructor of " + word);
        }

        String describe() { return word; }
    }

    static String say(String what) {
        System.out.println(what);
        return what;
    }

    public static void main(String[] args) {
        System.out.println("main starts");
        Step s = Step.THIRD;
        System.out.println("first use done");
        for (Step step : Step.values()) {
            System.out.println(step.ordinal() + " " + step.describe());
        }
        System.out.println(s + " " + Step.AFTER);
    }
}
