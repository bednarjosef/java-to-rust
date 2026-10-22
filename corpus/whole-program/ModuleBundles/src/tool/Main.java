package tool;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.util.Locale;
import java.util.MissingResourceException;
import java.util.PropertyResourceBundle;
import java.util.ResourceBundle;

// javac's diagnostics come from bundles of its own module, which ResourceBundle loads by name.
public class Main {
    public static void main(String[] args) throws IOException {
        ResourceBundle bundle = ResourceBundle.getBundle("tool.resources.messages", Locale.ROOT);
        System.out.println(bundle.getString("greeting") + " / " + bundle.getString("count"));
        System.out.println(bundle.getClass().getName() + " " + bundle.containsKey("count"));
        System.out.println(ResourceBundle.getBundle("tool.resources.messages", Locale.ROOT) == bundle);
        try {
            bundle.getString("missing");
        } catch (MissingResourceException e) {
            System.out.println(e.getMessage() + " " + e.getKey());
        }
        try {
            ResourceBundle.getBundle("tool.resources.absent", Locale.ROOT);
        } catch (MissingResourceException e) {
            System.out.println(e.getMessage());
        }
            // A bundle of properties, in UTF-8 or, if its bytes are not UTF-8, in ISO-8859-1.
        byte[] utf8 = "name = caf\u00e9\nother: \\u0041b\\\n  c\n# note\n".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        properties(utf8);
        properties(new byte[] {'k', '=', (byte) 0xe9, 'x', '\n', 'm', '=', (byte) 0xc3, (byte) 0xa9});
        properties(new byte[] {'k', '=', (byte) 0xc3, (byte) 0xa9, (byte) 0xe2, (byte) 0x82});
        properties(new byte[] {'k', '=', (byte) 0xed, (byte) 0xa0, (byte) 0x80});
    }

    static void properties(byte[] bytes) throws IOException {
        try {
            PropertyResourceBundle bundle = new PropertyResourceBundle(new ByteArrayInputStream(bytes));
            for (String key : new java.util.TreeSet<>(bundle.keySet())) {
                String value = bundle.getString(key);
                StringBuilder codes = new StringBuilder();
                for (int i = 0; i < value.length(); i++) {
                    codes.append(' ').append((int) value.charAt(i));
                }
                System.out.println(key + " =" + codes);
            }
        } catch (java.nio.charset.MalformedInputException e) {
            System.out.println("MalformedInputException: " + e.getMessage());
        }
    }
}
