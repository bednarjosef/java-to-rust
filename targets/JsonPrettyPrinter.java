// Features: a JSON-like value model (an abstract JsonValue with JsonObject, JsonArray, JsonString,
// JsonNumber and JsonLiteral subclasses), polymorphic render with an indent parameter, StringBuilder
// with repeated spaces and escaping of quotes, LinkedHashMap to keep insertion order, a recursive
// descent reader over a String with a position field, a checked ParseException carrying the offset,
// instanceof patterns and casts, and String.join for the key list.
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public class JsonPrettyPrinter {
    static class ParseException extends Exception {
        final int offset;

        ParseException(String message, int offset) {
            super(message + " at offset " + offset);
            this.offset = offset;
        }
    }

    static abstract class JsonValue {
        abstract void render(StringBuilder out, int indent);

        String pretty() {
            StringBuilder sb = new StringBuilder();
            render(sb, 0);
            return sb.toString();
        }
    }

    static class JsonString extends JsonValue {
        final String value;

        JsonString(String value) {
            this.value = value;
        }

        @Override
        void render(StringBuilder out, int indent) {
            out.append('"').append(value.replace("\"", "\\\"").replace("\n", "\\n")).append('"');
        }
    }

    static class JsonNumber extends JsonValue {
        final double value;

        JsonNumber(double value) {
            this.value = value;
        }

        @Override
        void render(StringBuilder out, int indent) {
            if (value == Math.floor(value)) {
                out.append((long) value);
            } else {
                out.append(value);
            }
        }
    }

    static class JsonLiteral extends JsonValue {
        final String text;

        JsonLiteral(String text) {
            this.text = text;
        }

        @Override
        void render(StringBuilder out, int indent) {
            out.append(text);
        }
    }

    static class JsonArray extends JsonValue {
        final List<JsonValue> items = new ArrayList<>();

        @Override
        void render(StringBuilder out, int indent) {
            if (items.isEmpty()) { out.append("[]"); return; }
            out.append("[\n");
            for (int i = 0; i < items.size(); i++) {
                out.append(" ".repeat(indent + 2));
                items.get(i).render(out, indent + 2);
                out.append(i + 1 < items.size() ? ",\n" : "\n");
            }
            out.append(" ".repeat(indent)).append(']');
        }
    }

    static class JsonObject extends JsonValue {
        final Map<String, JsonValue> fields = new LinkedHashMap<>();

        @Override
        void render(StringBuilder out, int indent) {
            if (fields.isEmpty()) { out.append("{}"); return; }
            out.append("{\n");
            int seen = 0;
            for (Map.Entry<String, JsonValue> e : fields.entrySet()) {
                out.append(" ".repeat(indent + 2)).append('"').append(e.getKey()).append("\": ");
                e.getValue().render(out, indent + 2);
                seen++;
                out.append(seen < fields.size() ? ",\n" : "\n");
            }
            out.append(" ".repeat(indent)).append('}');
        }
    }

    static class Reader {
        private final String text;
        private int pos;

        Reader(String text) {
            this.text = text;
        }

        private void skipSpaces() {
            while (pos < text.length() && Character.isWhitespace(text.charAt(pos))) {
                pos++;
            }
        }

        private boolean at(char c) {
            return pos < text.length() && text.charAt(pos) == c;
        }

        JsonValue read() throws ParseException {
            skipSpaces();
            if (pos >= text.length()) {
                throw new ParseException("unexpected end", pos);
            }
            char c = text.charAt(pos);
            if (c == '{') {
                return readObject();
            }
            if (c == '[') {
                return readArray();
            }
            if (c == '"') {
                return new JsonString(readString());
            }
            for (String literal : new String[] {"true", "false", "null"}) {
                if (text.startsWith(literal, pos)) {
                    pos += literal.length();
                    return new JsonLiteral(literal);
                }
            }
            if (c == '-' || Character.isDigit(c)) {
                int start = pos++;
                while (pos < text.length() && (Character.isDigit(text.charAt(pos)) || text.charAt(pos) == '.')) {
                    pos++;
                }
                return new JsonNumber(Double.parseDouble(text.substring(start, pos)));
            }
            throw new ParseException("unexpected character '" + c + "'", pos);
        }

        private String readString() throws ParseException {
            pos++;
            StringBuilder sb = new StringBuilder();
            while (pos < text.length() && !at('"')) {
                if (at('\\') && pos + 1 < text.length()) {
                    pos++;
                }
                sb.append(text.charAt(pos++));
            }
            if (pos >= text.length()) {
                throw new ParseException("unterminated string", pos);
            }
            pos++;
            return sb.toString();
        }

        private JsonValue readArray() throws ParseException {
            JsonArray array = new JsonArray();
            pos++;
            skipSpaces();
            if (at(']')) { pos++; return array; }
            while (true) {
                array.items.add(read());
                skipSpaces();
                if (at(',')) {
                    pos++;
                } else if (at(']')) {
                    pos++;
                    return array;
                } else {
                    throw new ParseException("expected , or ]", pos);
                }
            }
        }

        private JsonValue readObject() throws ParseException {
            JsonObject object = new JsonObject();
            pos++;
            skipSpaces();
            if (at('}')) { pos++; return object; }
            while (true) {
                skipSpaces();
                if (!at('"')) {
                    throw new ParseException("expected key", pos);
                }
                String key = readString();
                skipSpaces();
                if (!at(':')) {
                    throw new ParseException("expected colon", pos);
                }
                pos++;
                object.fields.put(key, read());
                skipSpaces();
                if (at(',')) {
                    pos++;
                } else if (at('}')) {
                    pos++;
                    return object;
                } else {
                    throw new ParseException("expected , or }", pos);
                }
            }
        }
    }

    public static void main(String[] args) {
        String source = "{\"name\": \"Widget \\\"Pro\\\"\", \"price\": 12.5, \"count\": 3, "
                + "\"tags\": [\"tools\", \"metal\"], \"meta\": {\"active\": true, \"note\": null, \"dims\": []}}";
        try {
            JsonValue doc = new Reader(source).read();
            System.out.println(doc.pretty());
            if (doc instanceof JsonObject obj) {
                System.out.println("keys: " + String.join(", ", obj.fields.keySet()));
            }
        } catch (ParseException e) {
            System.out.println("parse error: " + e.getMessage());
        }

        String[] broken = {"[1, 2", "{\"a\" 1}", "[true, @]", "\"open"};
        for (String text : broken) {
            try {
                new Reader(text).read();
                System.out.println("parsed unexpectedly: " + text);
            } catch (ParseException e) {
                System.out.println("error at " + e.offset + ": " + e.getMessage());
            }
        }
    }
}
