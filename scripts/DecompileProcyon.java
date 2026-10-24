// Decompiles classes of a jar with Procyon, for the survey's best-of step (see
// docs/minecraft-server.md and scripts/minecraft-server-survey.sh):
//
//   java -cp PROCYON_JARS scripts/DecompileProcyon.java JAR CLASSPATH OUT CLASS...
//
// Each CLASS is a top-level class's internal name (net/minecraft/util/Util); its source, with its
// nested classes, goes to OUT/<CLASS>.java. Procyon reads the classes the code uses from JAR,
// then from the jars of CLASSPATH (separated by ':'), then from the JDK, so that it knows their
// generic signatures. A class Procyon fails on is reported and left out. Procyon leaves `public`
// off a public record; the top-level declaration gets it back where the class file is public.

import com.strobel.assembler.InputTypeLoader;
import com.strobel.assembler.metadata.CompositeTypeLoader;
import com.strobel.assembler.metadata.ITypeLoader;
import com.strobel.assembler.metadata.JarTypeLoader;
import com.strobel.decompiler.Decompiler;
import com.strobel.decompiler.DecompilerSettings;
import com.strobel.decompiler.PlainTextOutput;
import java.io.StringWriter;
import java.lang.classfile.ClassFile;
import java.lang.reflect.AccessFlag;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.jar.JarFile;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class DecompileProcyon {
    public static void main(String[] args) throws Exception {
        List<ITypeLoader> loaders = new ArrayList<>();
        loaders.add(new JarTypeLoader(new JarFile(args[0])));
        for (String jar : args[1].split(":")) {
            if (!jar.isEmpty()) {
                loaders.add(new JarTypeLoader(new JarFile(jar)));
            }
        }
        loaders.add(new InputTypeLoader());
        DecompilerSettings settings = DecompilerSettings.javaDefaults();
        settings.setTypeLoader(new CompositeTypeLoader(loaders.toArray(new ITypeLoader[0])));
        JarFile jar = new JarFile(args[0]);
        for (int i = 3; i < args.length; i++) {
            Path out = Path.of(args[2], args[i] + ".java");
            StringWriter text = new StringWriter();
            try {
                Decompiler.decompile(args[i], new PlainTextOutput(text), settings);
            } catch (Exception | StackOverflowError e) {
                System.err.println(args[i] + ": " + e);
                continue;
            }
            Files.createDirectories(out.getParent());
            Files.writeString(out, publicAsInClassFile(text.toString(), jar, args[i]));
        }
    }

    /// The top-level declaration of a class's source, a line at the start of a line (`record
    /// Name<T>(..) {`, `final class Name {`), with `public` before it if the class file is public
    /// and the source does not say so.
    static String publicAsInClassFile(String source, JarFile jar, String name) throws Exception {
        var entry = jar.getEntry(name + ".class");
        if (entry == null) {
            return source;
        }
        byte[] bytes;
        try (var in = jar.getInputStream(entry)) {
            bytes = in.readAllBytes();
        }
        if (!ClassFile.of().parse(bytes).flags().has(AccessFlag.PUBLIC)) {
            return source;
        }
        String simple = name.substring(name.lastIndexOf('/') + 1);
        Matcher declaration = Pattern.compile(
                "^((?:abstract |final |sealed |non-sealed |strictfp )*(?:class|interface|enum|record|@interface) "
                        + Pattern.quote(simple) + "\\b)",
                Pattern.MULTILINE).matcher(source);
        return declaration.find() ? declaration.replaceFirst("public $1") : source;
    }
}
