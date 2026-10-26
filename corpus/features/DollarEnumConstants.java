// Enum constants whose names have a `$`, as the JDK's `KnownOIDs.AES_128$ECB$NoPadding`.
public class DollarEnumConstants {
    enum Mode {
        AES_128$ECB$NoPadding("ECB"),
        AES_128$CBC$NoPadding("CBC"),
        AES_128_CBC("plain"),
        $START("start");

        final String text;

        Mode(String text) {
            this.text = text;
        }
    }

    public static void main(String[] args) {
        for (Mode m : Mode.values()) {
            System.out.println(m + " " + m.text + " " + m.ordinal());
        }
        System.out.println(Mode.valueOf("AES_128$CBC$NoPadding").text);
        Mode m = Mode.$START;
        switch (m) {
            case AES_128$ECB$NoPadding -> System.out.println("ecb");
            case $START -> System.out.println("first");
            default -> System.out.println("other");
        }
    }
}
