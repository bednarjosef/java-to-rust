import java.lang.reflect.AccessibleObject;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.Arrays;

// An enum's constants found as fields, as gson's `EnumTypeAdapter` finds them: the declared
// fields that are enum constants, made accessible and read, with their modifiers.
public class EnumConstantFields {
    enum Mode {
        SURVIVAL("s"),
        CREATIVE("c") {
            @Override
            String label() {
                return "creative!";
            }
        },
        SPECTATOR("v");

        static final int COUNT = 3;
        final String letter;

        Mode(String letter) {
            this.letter = letter;
        }

        String label() {
            return letter;
        }
    }

    public static void main(String[] args) throws Exception {
        Field[] fields = Mode.class.getDeclaredFields();
        int constants = 0;
        for (Field f : fields) {
            if (f.isEnumConstant()) {
                fields[constants++] = f;
            }
        }
        fields = Arrays.copyOf(fields, constants);
        AccessibleObject.setAccessible(fields, true);
        for (Field f : fields) {
            Mode constant = (Mode) f.get(null);
            System.out.println(f.getName() + " " + constant.name() + " " + constant.label() + " " + constant.letter
                    + " " + Modifier.toString(f.getModifiers()) + " " + f.getType().getSimpleName());
        }
        Field count = Mode.class.getDeclaredField("COUNT");
        System.out.println(count.isEnumConstant() + " " + count.get(null));
        Field creative = Mode.class.getField("CREATIVE");
        System.out.println(creative.isEnumConstant() + " " + (creative.get(null) == Mode.CREATIVE));
        try {
            creative.set(null, Mode.SURVIVAL);
        } catch (IllegalAccessException e) {
            System.out.println("final");
        }
    }
}
