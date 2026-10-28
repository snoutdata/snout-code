package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.List;
import java.util.Set;

/**
 * Finds the test classes and methods of a Java or Scala file without compiling it, for the
 * editor's gutter run icons: a token scan that skips comments and string literals, tracks the
 * package and nested types by braces, and takes a method as a test when a test annotation
 * precedes it (JUnit 4 and 5, TestNG). A class is a test class when it, or a type nested in it,
 * has a test method, or when it carries a class-level TestNG {@code @Test} or {@code @RunWith}.
 * JUnit 3 classes ({@code extends TestCase}) count their {@code public void test*()} methods.
 *
 * <p>Ranges are LSP ranges (0-based lines, UTF-16 columns) of the type's or method's name.
 */
final class TestDiscovery {
    private static final Set<String> TEST_ANNOTATIONS = Set.of(
        "Test", "ParameterizedTest", "RepeatedTest", "TestFactory", "TestTemplate", "Property", "Example");
    private static final Set<String> TYPE_KEYWORDS = Set.of("class", "interface", "enum", "record", "object", "trait");

    /** A test method, or a test type with its methods and nested test types. */
    static final class TestItem {
        String name;
        String qualifiedName;
        int line;
        int column;
        int length;
        boolean nested;
        final List<TestItem> methods = new ArrayList<>();
        final List<TestItem> types = new ArrayList<>();
        boolean classLevelTest;
        boolean junit3;
        boolean isAbstract;

        boolean isTestClass() {
            return !isAbstract && (classLevelTest || !methods.isEmpty() || types.stream().anyMatch(TestItem::isTestClass));
        }
    }

    private record Token(String text, int line, int column, boolean word) {
    }

    private TestDiscovery() {
    }

    /** The test types of a file, outermost first, as JSON for {@code snout/testsInFile}. */
    static JsonArray find(String text, boolean scala) {
        JsonArray out = new JsonArray();
        for (TestItem type : parse(text, scala)) {
            addType(out, type);
        }
        return out;
    }

    private static void addType(JsonArray out, TestItem type) {
        if (!type.isTestClass()) {
            return;
        }
        JsonObject t = new JsonObject();
        t.addProperty("name", type.qualifiedName);
        t.addProperty("simpleName", type.name);
        t.add("range", range(type));
        JsonArray methods = new JsonArray();
        for (TestItem m : type.methods) {
            JsonObject method = new JsonObject();
            method.addProperty("name", m.name);
            method.add("range", range(m));
            methods.add(method);
        }
        t.add("methods", methods);
        out.add(t);
        for (TestItem nested : type.types) {
            addType(out, nested);
        }
    }

    private static JsonObject range(TestItem item) {
        JsonObject start = new JsonObject();
        start.addProperty("line", item.line);
        start.addProperty("character", item.column);
        JsonObject end = new JsonObject();
        end.addProperty("line", item.line);
        end.addProperty("character", item.column + item.length);
        JsonObject range = new JsonObject();
        range.add("start", start);
        range.add("end", end);
        return range;
    }

    /** Every top-level type of the file, with what it holds. */
    static List<TestItem> parse(String text, boolean scala) {
        List<Token> tokens = tokenize(text, scala);
        List<TestItem> top = new ArrayList<>();
        String pkg = "";
        // The types whose bodies we are in, with the brace depth their body opened at.
        Deque<TestItem> types = new ArrayDeque<>();
        Deque<Integer> typeDepths = new ArrayDeque<>();
        int depth = 0;
        List<String> annotations = new ArrayList<>();
        List<String> modifiers = new ArrayList<>();
        TestItem pendingType = null;
        for (int i = 0; i < tokens.size(); i++) {
            Token t = tokens.get(i);
            String s = t.text();
            if (s.equals("package") && depth == 0 && types.isEmpty()) {
                StringBuilder name = new StringBuilder();
                int j = qualifiedName(tokens, i + 1, name);
                // Scala allows several package clauses; they nest.
                pkg = pkg.isEmpty() || !scala ? name.toString() : pkg + "." + name;
                i = j - 1;
                continue;
            }
            if (s.equals("@")) {
                // @Name, @a.b.Name, @Name(...): keep the simple name, skip the arguments.
                StringBuilder qualified = new StringBuilder();
                int j = qualifiedName(tokens, i + 1, qualified);
                String last = qualified.isEmpty() ? null : qualified.substring(qualified.lastIndexOf(".") + 1);
                if (last != null && !last.equals("interface")) {
                    annotations.add(last);
                }
                if (j < tokens.size() && tokens.get(j).text().equals("(")) {
                    j = skipParens(tokens, j);
                }
                i = j - 1;
                continue;
            }
            if (s.equals("{")) {
                depth++;
                if (pendingType != null) {
                    types.push(pendingType);
                    typeDepths.push(depth);
                    pendingType = null;
                }
                annotations.clear();
                modifiers.clear();
                continue;
            }
            if (s.equals("}")) {
                if (!typeDepths.isEmpty() && typeDepths.peek() == depth) {
                    typeDepths.pop();
                    types.pop();
                }
                depth--;
                annotations.clear();
                modifiers.clear();
                continue;
            }
            if (s.equals(";")) {
                annotations.clear();
                modifiers.clear();
                if (pendingType != null && !scala) {
                    pendingType = null;
                }
                continue;
            }
            if (t.word() && TYPE_KEYWORDS.contains(s) && i + 1 < tokens.size() && tokens.get(i + 1).word()
                && !(i > 0 && tokens.get(i - 1).text().equals("."))) {
                Token name = tokens.get(i + 1);
                TestItem type = new TestItem();
                type.name = name.text();
                type.line = name.line();
                type.column = name.column();
                type.length = name.text().length();
                type.isAbstract = modifiers.contains("abstract") || s.equals("interface") || s.equals("trait");
                type.classLevelTest = annotations.contains("RunWith") || (annotations.contains("Test") && !scala);
                TestItem outer = types.peek();
                if (outer == null) {
                    type.qualifiedName = pkg.isEmpty() ? type.name : pkg + "." + type.name;
                    top.add(type);
                } else {
                    type.nested = true;
                    type.qualifiedName = outer.qualifiedName + "$" + type.name;
                    outer.types.add(type);
                }
                // JUnit 3: extends TestCase, up to the body.
                for (int j = i + 2; j < tokens.size() && !tokens.get(j).text().equals("{") && !tokens.get(j).text().equals(";"); j++) {
                    if (tokens.get(j).text().equals("TestCase")) {
                        type.junit3 = true;
                    }
                }
                pendingType = type;
                annotations.clear();
                modifiers.clear();
                i++;
                continue;
            }
            if (t.word() && isModifier(s)) {
                modifiers.add(s);
                continue;
            }
            if (scala && pendingType != null && (s.equals("def") || s.equals("val") || s.equals("var"))) {
                pendingType = null; // the type had no body: `case class Point(x: Int)`
            }
            TestItem owner = types.peek();
            if (owner == null || typeDepths.peek() != depth) {
                continue; // not directly in a type body
            }
            // A method: `def name` in Scala; `name (` after a type in Java.
            Token method = null;
            if (scala && s.equals("def") && i + 1 < tokens.size() && tokens.get(i + 1).word()) {
                method = tokens.get(i + 1);
                i++;
            } else if (!scala && t.word() && i + 1 < tokens.size() && tokens.get(i + 1).text().equals("(")
                && i > 0 && (tokens.get(i - 1).word() || tokens.get(i - 1).text().equals(">") || tokens.get(i - 1).text().equals("]"))
                && !s.equals("new") && !isKeyword(tokens.get(i - 1).text())) {
                method = t;
            } else if (!scala && t.word() && i + 1 < tokens.size() && tokens.get(i + 1).text().equals("(")) {
                // A call or a constructor: not a declaration we track.
                annotations.clear();
            }
            if (method != null) {
                boolean test = annotations.stream().anyMatch(TEST_ANNOTATIONS::contains)
                    || (owner.junit3 && method.text().startsWith("test") && modifiers.contains("public"))
                    || (owner.classLevelTest && annotations.isEmpty() && modifiers.contains("public") && !modifiers.contains("static"));
                if (test) {
                    TestItem m = new TestItem();
                    m.name = method.text();
                    m.line = method.line();
                    m.column = method.column();
                    m.length = method.text().length();
                    if (owner.methods.stream().noneMatch(x -> x.name.equals(m.name))) {
                        owner.methods.add(m);
                    }
                }
                annotations.clear();
                modifiers.clear();
                if (!scala) {
                    // Skip the parameter list, so its annotations and names are not taken for members.
                    i = skipParens(tokens, i + 1) - 1;
                }
            }
        }
        return top;
    }

    private static boolean isModifier(String s) {
        return switch (s) {
            case "public", "protected", "private", "static", "final", "abstract", "synchronized", "native",
                 "default", "sealed", "override", "implicit", "lazy", "case", "strictfp", "transient", "volatile" -> true;
            default -> false;
        };
    }

    private static boolean isKeyword(String s) {
        return switch (s) {
            case "return", "throw", "else", "case", "new", "assert", "yield" -> true;
            default -> false;
        };
    }

    /** Reads {@code a.b.c} from {@code start} into {@code out}; returns the index after it. */
    private static int qualifiedName(List<Token> tokens, int start, StringBuilder out) {
        int j = start;
        if (j >= tokens.size() || !tokens.get(j).word()) {
            return j;
        }
        out.append(tokens.get(j).text());
        j++;
        while (j + 1 < tokens.size() && tokens.get(j).text().equals(".") && tokens.get(j + 1).word()) {
            out.append('.').append(tokens.get(j + 1).text());
            j += 2;
        }
        return j;
    }

    /** From an opening parenthesis, the index just after its match. */
    private static int skipParens(List<Token> tokens, int open) {
        int level = 0;
        for (int j = open; j < tokens.size(); j++) {
            String s = tokens.get(j).text();
            if (s.equals("(")) {
                level++;
            } else if (s.equals(")")) {
                level--;
                if (level == 0) {
                    return j + 1;
                }
            }
        }
        return tokens.size();
    }

    /** Words and single punctuation characters, with comments, strings and characters dropped. */
    private static List<Token> tokenize(String text, boolean scala) {
        List<Token> out = new ArrayList<>();
        int line = 0;
        int lineStart = 0;
        int n = text.length();
        int i = 0;
        while (i < n) {
            char c = text.charAt(i);
            if (c == '\n') {
                line++;
                i++;
                lineStart = i;
                continue;
            }
            if (Character.isWhitespace(c)) {
                i++;
                continue;
            }
            if (c == '/' && i + 1 < n && text.charAt(i + 1) == '/') {
                while (i < n && text.charAt(i) != '\n') {
                    i++;
                }
                continue;
            }
            if (c == '/' && i + 1 < n && text.charAt(i + 1) == '*') {
                // Scala block comments nest; Java's do not.
                int level = 0;
                while (i < n) {
                    if (text.startsWith("/*", i)) {
                        level++;
                        i += 2;
                        if (!scala && level > 1) {
                            level = 1;
                        }
                    } else if (text.startsWith("*/", i)) {
                        level--;
                        i += 2;
                        if (level == 0) {
                            break;
                        }
                    } else {
                        if (text.charAt(i) == '\n') {
                            line++;
                            lineStart = i + 1;
                        }
                        i++;
                    }
                }
                continue;
            }
            if (text.startsWith("\"\"\"", i)) {
                // A Java text block or a Scala multi-line string.
                int end = text.indexOf("\"\"\"", i + 3);
                end = end < 0 ? n : end + 3;
                while (end < n && text.charAt(end) == '"') {
                    end++;
                }
                for (int j = i; j < end; j++) {
                    if (text.charAt(j) == '\n') {
                        line++;
                        lineStart = j + 1;
                    }
                }
                i = end;
                continue;
            }
            if (c == '"' || (c == '\'' && !(scala && isScalaSymbol(text, i)))) {
                int j = i + 1;
                while (j < n && text.charAt(j) != c && text.charAt(j) != '\n') {
                    j += text.charAt(j) == '\\' ? 2 : 1;
                }
                i = Math.min(n, j + 1);
                continue;
            }
            if (Character.isJavaIdentifierStart(c)) {
                int j = i + 1;
                while (j < n && Character.isJavaIdentifierPart(text.charAt(j))) {
                    j++;
                }
                out.add(new Token(text.substring(i, j), line, i - lineStart, true));
                i = j;
                continue;
            }
            if (scala && c == '`') {
                int j = text.indexOf('`', i + 1);
                if (j > 0 && text.lastIndexOf('\n', j) < i) {
                    out.add(new Token(text.substring(i + 1, j), line, i - lineStart + 1, true));
                    i = j + 1;
                    continue;
                }
            }
            if (Character.isDigit(c)) {
                int j = i + 1;
                while (j < n && (Character.isLetterOrDigit(text.charAt(j)) || text.charAt(j) == '.' || text.charAt(j) == '_')) {
                    j++;
                }
                i = j;
                continue;
            }
            out.add(new Token(String.valueOf(c), line, i - lineStart, false));
            i++;
        }
        return out;
    }

    /** A Scala symbol literal ({@code 'name}) rather than a character literal ({@code 'c'}). */
    private static boolean isScalaSymbol(String text, int i) {
        if (i + 2 < text.length() && text.charAt(i + 2) == '\'') {
            return false;
        }
        return i + 1 < text.length() && text.charAt(i + 1) != '\\' && Character.isJavaIdentifierStart(text.charAt(i + 1))
            && !(i + 3 < text.length() && text.charAt(i + 3) == '\'');
    }
}
