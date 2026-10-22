package tool.resources;

import java.util.ListResourceBundle;

// A bundle as javac's are: a class that the build makes from a file of properties.
public final class messages extends ListResourceBundle {
    protected final Object[][] getContents() {
        return new Object[][] {
            {"greeting", "hello, {0}"},
            {"count", "{0} errors"},
        };
    }
}
