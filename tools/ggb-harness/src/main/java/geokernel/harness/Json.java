package geokernel.harness;

import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Minimal JSON reader and writer, enough for the harness input and the golden
 * files. Objects become {@link LinkedHashMap} (key order is kept), arrays
 * become {@link List}, numbers become {@link Double} or {@link Long}.
 */
final class Json {

    private final String text;
    private int pos;

    private Json(String text) {
        this.text = text;
    }

    /** Parses one JSON value that spans the whole text. */
    static Object parse(String text) {
        Json p = new Json(text);
        p.skipSpace();
        Object value = p.readValue();
        p.skipSpace();
        if (p.pos != text.length()) {
            throw p.error("trailing characters");
        }
        return value;
    }

    private Object readValue() {
        if (pos >= text.length()) {
            throw error("unexpected end of input");
        }
        char c = text.charAt(pos);
        switch (c) {
            case '{':
                return readObject();
            case '[':
                return readArray();
            case '"':
                return readString();
            case 't':
                expectWord("true");
                return Boolean.TRUE;
            case 'f':
                expectWord("false");
                return Boolean.FALSE;
            case 'n':
                expectWord("null");
                return null;
            default:
                return readNumber();
        }
    }

    private Map<String, Object> readObject() {
        Map<String, Object> map = new LinkedHashMap<>();
        pos++;
        skipSpace();
        if (peek() == '}') {
            pos++;
            return map;
        }
        while (true) {
            skipSpace();
            String key = readString();
            skipSpace();
            expect(':');
            skipSpace();
            if (map.containsKey(key)) {
                throw error("duplicate key " + key);
            }
            map.put(key, readValue());
            skipSpace();
            if (peek() == ',') {
                pos++;
                continue;
            }
            expect('}');
            return map;
        }
    }

    private List<Object> readArray() {
        List<Object> list = new ArrayList<>();
        pos++;
        skipSpace();
        if (peek() == ']') {
            pos++;
            return list;
        }
        while (true) {
            skipSpace();
            list.add(readValue());
            skipSpace();
            if (peek() == ',') {
                pos++;
                continue;
            }
            expect(']');
            return list;
        }
    }

    private String readString() {
        expect('"');
        StringBuilder sb = new StringBuilder();
        while (true) {
            if (pos >= text.length()) {
                throw error("unterminated string");
            }
            char c = text.charAt(pos++);
            if (c == '"') {
                return sb.toString();
            }
            if (c != '\\') {
                sb.append(c);
                continue;
            }
            char e = text.charAt(pos++);
            switch (e) {
                case '"':
                case '\\':
                case '/':
                    sb.append(e);
                    break;
                case 'n':
                    sb.append('\n');
                    break;
                case 't':
                    sb.append('\t');
                    break;
                case 'r':
                    sb.append('\r');
                    break;
                case 'b':
                    sb.append('\b');
                    break;
                case 'f':
                    sb.append('\f');
                    break;
                case 'u':
                    sb.append((char) Integer.parseInt(text.substring(pos, pos + 4), 16));
                    pos += 4;
                    break;
                default:
                    throw error("bad escape");
            }
        }
    }

    private Object readNumber() {
        int start = pos;
        while (pos < text.length() && "+-0123456789.eE".indexOf(text.charAt(pos)) >= 0) {
            pos++;
        }
        String s = text.substring(start, pos);
        if (s.isEmpty()) {
            throw error("unexpected character");
        }
        if (s.matches("-?\\d+")) {
            return Long.parseLong(s);
        }
        return Double.parseDouble(s);
    }

    private void expectWord(String word) {
        if (!text.startsWith(word, pos)) {
            throw error("expected " + word);
        }
        pos += word.length();
    }

    private void expect(char c) {
        if (peek() != c) {
            throw error("expected '" + c + "'");
        }
        pos++;
    }

    private char peek() {
        return pos < text.length() ? text.charAt(pos) : '\0';
    }

    private void skipSpace() {
        while (pos < text.length() && Character.isWhitespace(text.charAt(pos))) {
            pos++;
        }
    }

    private IllegalArgumentException error(String message) {
        return new IllegalArgumentException("JSON: " + message + " at offset " + pos);
    }

    // ---------------------------------------------------------------- writer

    /**
     * Writes a value with two-space indentation. Objects whose values are all
     * scalars are written on one line; every other object and every non-empty
     * array is spread over several lines.
     */
    static String write(Object value) {
        StringBuilder sb = new StringBuilder();
        writeValue(sb, value, 0);
        sb.append('\n');
        return sb.toString();
    }

    /** Writes a value on one line with no padding, for a JSON Lines file. */
    static String writeLine(Object value) {
        StringBuilder sb = new StringBuilder();
        writeCompact(sb, value);
        return sb.toString();
    }

    private static void writeCompact(StringBuilder sb, Object value) {
        if (value instanceof Map) {
            sb.append('{');
            boolean first = true;
            for (Map.Entry<?, ?> e : ((Map<?, ?>) value).entrySet()) {
                if (!first) {
                    sb.append(',');
                }
                first = false;
                writeString(sb, (String) e.getKey());
                sb.append(':');
                writeCompact(sb, e.getValue());
            }
            sb.append('}');
        } else if (value instanceof List) {
            sb.append('[');
            boolean first = true;
            for (Object v : (List<?>) value) {
                if (!first) {
                    sb.append(',');
                }
                first = false;
                writeCompact(sb, v);
            }
            sb.append(']');
        } else {
            writeScalar(sb, value);
        }
    }

    private static void writeValue(StringBuilder sb, Object value, int indent) {
        if (value instanceof Map) {
            writeObject(sb, (Map<?, ?>) value, indent);
        } else if (value instanceof List) {
            writeArray(sb, (List<?>) value, indent);
        } else {
            writeScalar(sb, value);
        }
    }

    private static void writeObject(StringBuilder sb, Map<?, ?> map, int indent) {
        if (map.isEmpty()) {
            sb.append("{}");
            return;
        }
        boolean flat = map.values().stream()
                .noneMatch(v -> v instanceof Map || v instanceof List);
        if (flat) {
            sb.append("{ ");
            boolean first = true;
            for (Map.Entry<?, ?> e : map.entrySet()) {
                if (!first) {
                    sb.append(", ");
                }
                first = false;
                writeString(sb, (String) e.getKey());
                sb.append(": ");
                writeScalar(sb, e.getValue());
            }
            sb.append(" }");
            return;
        }
        sb.append("{\n");
        int i = 0;
        for (Map.Entry<?, ?> e : map.entrySet()) {
            pad(sb, indent + 1);
            writeString(sb, (String) e.getKey());
            sb.append(": ");
            writeValue(sb, e.getValue(), indent + 1);
            sb.append(++i < map.size() ? ",\n" : "\n");
        }
        pad(sb, indent);
        sb.append('}');
    }

    private static void writeArray(StringBuilder sb, List<?> list, int indent) {
        if (list.isEmpty()) {
            sb.append("[]");
            return;
        }
        // A list of numbers (a vertex `[x, y]`, or a function sample `[x, y]`
        // whose `y` may be null) stays on one line.
        if (list.stream().allMatch(v -> v == null || v instanceof Double)
                && list.stream().anyMatch(v -> v instanceof Double)) {
            sb.append('[');
            for (int i = 0; i < list.size(); i++) {
                if (i > 0) {
                    sb.append(", ");
                }
                writeScalar(sb, list.get(i));
            }
            sb.append(']');
            return;
        }
        sb.append("[\n");
        for (int i = 0; i < list.size(); i++) {
            pad(sb, indent + 1);
            writeValue(sb, list.get(i), indent + 1);
            sb.append(i + 1 < list.size() ? ",\n" : "\n");
        }
        pad(sb, indent);
        sb.append(']');
    }

    private static void writeScalar(StringBuilder sb, Object value) {
        if (value == null) {
            sb.append("null");
        } else if (value instanceof String) {
            writeString(sb, (String) value);
        } else if (value instanceof Boolean || value instanceof Long
                || value instanceof Integer) {
            sb.append(value);
        } else if (value instanceof Double) {
            sb.append(formatDouble((Double) value));
        } else {
            throw new IllegalArgumentException("cannot write " + value.getClass());
        }
    }

    /** Formats a finite double so that parsing the text gives the same bits. */
    static String formatDouble(double d) {
        if (!Double.isFinite(d)) {
            throw new IllegalArgumentException("non-finite number " + d);
        }
        String s = Double.toString(d);
        if (Double.doubleToRawLongBits(Double.parseDouble(s)) != Double.doubleToRawLongBits(d)) {
            s = new BigDecimal(d).toString();
        }
        return s;
    }

    private static void writeString(StringBuilder sb, String s) {
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '"' || c == '\\') {
                sb.append('\\').append(c);
            } else if (c < 0x20) {
                sb.append(String.format("\\u%04x", (int) c));
            } else {
                sb.append(c);
            }
        }
        sb.append('"');
    }

    private static void pad(StringBuilder sb, int indent) {
        for (int i = 0; i < indent; i++) {
            sb.append("  ");
        }
    }
}
