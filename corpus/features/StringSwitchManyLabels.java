// A switch on a string with more labels than the generator matches as bytes: it matches the
// number of the label's case instead. Each form of switch, a null selector and a label that no
// case has.
public class StringSwitchManyLabels {
    static String kind(String s) {
        switch (s) {
            case "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10", "k11", "k12", "k13", "k14", "k15", "k16", "k17", "k18", "k19", "k20", "k21", "k22", "k23", "k24", "k25", "k26", "k27", "k28", "k29", "k30", "k31", "k32", "k33", "k34", "k35", "k36", "k37", "k38", "k39":
                return "first";
            case "k40", "k41", "k42", "k43", "k44", "k45", "k46", "k47", "k48", "k49", "k50", "k51", "k52", "k53", "k54", "k55", "k56", "k57", "k58", "k59", "k60", "k61", "k62", "k63", "k64", "k65", "k66", "k67", "k68":
                return "second";
            case "k69", "caf\u00e9":
                return "last";
            default:
                return "none";
        }
    }

    static int value(String s) {
        return switch (s) {
            case "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10", "k11", "k12", "k13", "k14", "k15", "k16", "k17", "k18", "k19", "k20", "k21", "k22", "k23", "k24", "k25", "k26", "k27", "k28", "k29", "k30", "k31", "k32", "k33", "k34", "k35", "k36", "k37", "k38", "k39" -> 1;
            case "k40", "k41", "k42", "k43", "k44", "k45", "k46", "k47", "k48", "k49", "k50", "k51", "k52", "k53", "k54", "k55", "k56", "k57", "k58", "k59", "k60", "k61", "k62", "k63", "k64", "k65", "k66", "k67", "k68" -> {
                int n = s.length();
                yield n * 10;
            }
            default -> -1;
        };
    }

    static int fallThrough(String s) {
        int n = 0;
        switch (s) {
            case "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10", "k11", "k12", "k13", "k14", "k15", "k16", "k17", "k18", "k19", "k20", "k21", "k22", "k23", "k24", "k25", "k26", "k27", "k28", "k29", "k30", "k31", "k32", "k33", "k34", "k35", "k36", "k37", "k38", "k39":
                n += 1;
            case "k40", "k41", "k42", "k43", "k44", "k45", "k46", "k47", "k48", "k49", "k50", "k51", "k52", "k53", "k54", "k55", "k56", "k57", "k58", "k59", "k60", "k61", "k62", "k63", "k64", "k65", "k66", "k67", "k68":
                n += 10;
                break;
            case "k69":
                n += 100;
            default:
                n += 1000;
        }
        return n;
    }

    static String early(String s, boolean stop) {
        String out = "start";
        found:
        switch (s) {
            case "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9", "k10", "k11", "k12", "k13", "k14", "k15", "k16", "k17", "k18", "k19", "k20", "k21", "k22", "k23", "k24", "k25", "k26", "k27", "k28", "k29", "k30", "k31", "k32", "k33", "k34", "k35", "k36", "k37", "k38", "k39":
                if (stop) {
                    break found;
                }
                out = "first";
                break;
            case "k40", "k41", "k42", "k43", "k44", "k45", "k46", "k47", "k48", "k49", "k50", "k51", "k52", "k53", "k54", "k55", "k56", "k57", "k58", "k59", "k60", "k61", "k62", "k63", "k64", "k65", "k66", "k67", "k68":
                out = "second";
                break;
        }
        return out;
    }

    public static void main(String[] args) {
        String[] names = { "k0", "k39", "k40", "k68", "k69", "caf\u00e9", "k70", "", "K1" };
        for (String name : names) {
            System.out.println(name + " " + kind(name) + " " + value(name) + " " + fallThrough(name) + " "
                + early(name, false) + " " + early(name, true));
        }
        try {
            System.out.println(kind(null));
        } catch (NullPointerException e) {
            System.out.println("null selector");
        }
    }
}
