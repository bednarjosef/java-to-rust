import java.lang.module.ModuleDescriptor;
import java.lang.reflect.AccessFlag;
import java.lang.reflect.ClassFileFormatVersion;
import java.util.TreeSet;

// The access flags of class files as values, as `ModuleDescriptor.Modifier`'s constants are made
// of them (`OPEN(AccessFlag.OPEN.mask())`): the enums of `java.lang.reflect` that name a class
// file's flags and versions.
public class AccessFlagValues {
    public static void main(String[] args) {
        for (ModuleDescriptor.Modifier m : ModuleDescriptor.Modifier.values()) {
            System.out.println(m + " " + m.ordinal());
        }
        System.out.println(AccessFlag.PUBLIC.mask() + " " + AccessFlag.OPEN.mask() + " " + AccessFlag.SYNTHETIC.mask());
        System.out.println(AccessFlag.STATIC.sourceModifier() + " " + AccessFlag.SYNTHETIC.sourceModifier());
        System.out.println(new TreeSet<>(AccessFlag.maskToAccessFlags(0x1011, AccessFlag.Location.CLASS)));
        System.out.println(new TreeSet<>(AccessFlag.STATIC.locations()));
        System.out.println(new TreeSet<>(AccessFlag.STATIC.locations(ClassFileFormatVersion.RELEASE_8)));
        try {
            AccessFlag.maskToAccessFlags(0x0008, AccessFlag.Location.CLASS);
        } catch (IllegalArgumentException e) {
            System.out.println("not a class flag");
        }
        System.out.println(ClassFileFormatVersion.RELEASE_17.major() + " " + ClassFileFormatVersion.RELEASE_17.runtimeVersion());
    }
}
