// Manifests read from bytes: the attributes of a good one, and the error of a bad one, whose
// message says where (`SecurityProperties.INCLUDE_JAR_NAME_IN_EXCEPTIONS`).
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.jar.Attributes;
import java.util.jar.Manifest;

public class JarManifests {
    public static void main(String[] args) throws Exception {
        try {
            new Manifest(new ByteArrayInputStream("Manifest-Version 1.0\n".getBytes()));
        } catch (IOException e) {
            System.out.println(e.getMessage());
        }
        Manifest manifest = new Manifest();
        manifest.getMainAttributes().put(Attributes.Name.MANIFEST_VERSION, "1.0");
        manifest.getMainAttributes().putValue("Created-By", "a test");
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        manifest.write(bytes);
        Manifest read = new Manifest(new ByteArrayInputStream(bytes.toByteArray()));
        System.out.println(read.getMainAttributes().getValue("Created-By") + " " + read.equals(manifest));
    }
}
