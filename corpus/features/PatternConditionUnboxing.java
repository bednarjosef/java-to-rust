public class PatternConditionUnboxing {
    static class Level {}

    static class ServerLevel extends Level {
        Rules rules = new Rules();
    }

    static class Key<T> {
        T value;

        Key(T value) { this.value = value; }
    }

    static class Rules {
        <T> T get(Key<T> key) { return key.value; }
    }

    static final Key<Boolean> GRIEFING = new Key<>(true);
    static final Key<Boolean> UNSET = new Key<>(null);
    static Level current = new ServerLevel();

    boolean flag = true;

    Level level() { return current; }

    boolean step(Key<Boolean> key) {
        if (this.level() instanceof ServerLevel serverLevel && this.flag && serverLevel.rules.get(key)) {
            return true;
        }
        return false;
    }

    public static void main(String[] a) {
        try {
            System.out.println(new PatternConditionUnboxing().step(GRIEFING));
            System.out.println(new PatternConditionUnboxing().step(UNSET));
        } catch (NullPointerException e) {
            System.out.println("npe " + e.getMessage());
        }
    }
}
