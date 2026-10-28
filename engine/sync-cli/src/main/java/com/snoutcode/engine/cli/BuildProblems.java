package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Reads compiler problems out of Gradle's output, line by line, for {@code snout/buildProblems}:
 *
 * <ul>
 *   <li>javac: {@code /src/App.java:12: error: cannot find symbol}, then the source line, a caret
 *       under the column, and indented details ({@code symbol:}, {@code location:});</li>
 *   <li>scalac through Gradle's Zinc: {@code [Error] /src/Foo.scala:12:5: not found: value x}
 *       (also {@code [Warn]}, {@code [Info]}), then the source line and a caret;</li>
 *   <li>older scalac: {@code /src/Foo.scala:12: error: not found: value x}, as javac;</li>
 *   <li>kotlinc: {@code e: file:///src/Foo.kt:12:5 message} and {@code e: /src/Foo.kt: (12, 5): message}.</li>
 * </ul>
 *
 * Lines and columns are 1-based, as the compilers print them; column 0 means unknown.
 */
final class BuildProblems {
    /** One problem. */
    record Problem(String file, int line, int column, String severity, String message) {
        JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("file", file);
            o.addProperty("line", line);
            o.addProperty("column", column);
            o.addProperty("severity", severity);
            o.addProperty("message", message);
            return o;
        }
    }

    private static final String SOURCE = "(.+?\\.(?:java|scala|sc|kt|kts|groovy))";
    private static final Pattern JAVAC = Pattern.compile("^" + SOURCE + ":(\\d+): (error|warning|warn|note): ?(.*)$");
    private static final Pattern ZINC = Pattern.compile("^\\[(Error|Warn|Warning|Info)\\] " + SOURCE + ":(\\d+):(?:(\\d+):)? ?(.*)$");
    private static final Pattern KOTLIN = Pattern.compile("^([ewi]): (?:file://)?" + SOURCE + ":(\\d+):(\\d+) (.*)$");
    private static final Pattern KOTLIN_OLD = Pattern.compile("^([ewi]): " + SOURCE + ": \\((\\d+), (\\d+)\\): (.*)$");
    private static final Pattern CARET = Pattern.compile("^(\\s*)\\^\\s*$");
    /** Lines that end a problem's continuation: Gradle's own output and compiler summaries. */
    private static final Pattern END = Pattern.compile(
        "^(> |FAILURE|BUILD |\\* |\\d+ (errors?|warnings?)$|(one|two|three|four) (errors?|warnings?) found|\\[(Error|Warn|Warning|Info)\\] |Note: |e: |w: )");

    private final List<Problem> problems = new ArrayList<>();
    private Pending pending;

    /** A problem whose continuation lines are still coming. */
    private static final class Pending {
        String file;
        int line;
        int column;
        String severity;
        String message;
        final List<String> rest = new ArrayList<>();
        boolean caretSeen;
        final List<String> details = new ArrayList<>();
    }

    /** Feeds one line of output (of one stream). */
    void accept(String text) {
        String line = stripAnsi(text);
        Matcher m;
        if ((m = ZINC.matcher(line)).matches()) {
            start(m.group(2), m.group(3), m.group(4), severity(m.group(1)), m.group(5));
        } else if ((m = JAVAC.matcher(line)).matches()) {
            start(m.group(1), m.group(2), null, severity(m.group(3)), m.group(4));
        } else if ((m = KOTLIN.matcher(line)).matches() || (m = KOTLIN_OLD.matcher(line)).matches()) {
            start(m.group(2), m.group(3), m.group(4), severity(m.group(1)), m.group(5));
        } else if (pending != null) {
            if (line.isBlank() || END.matcher(line).find()) {
                flush();
            } else {
                continuation(line);
            }
        }
    }

    /** Ends the input and returns every problem, in order. */
    List<Problem> finish() {
        flush();
        return problems;
    }

    static JsonArray toJson(List<Problem> problems) {
        JsonArray out = new JsonArray();
        problems.forEach(p -> out.add(p.toJson()));
        return out;
    }

    private void start(String file, String line, String column, String severity, String message) {
        flush();
        pending = new Pending();
        pending.file = file.trim();
        pending.line = Integer.parseInt(line);
        pending.column = column == null ? 0 : Integer.parseInt(column);
        pending.severity = severity;
        pending.message = message.trim();
    }

    private void continuation(String line) {
        Matcher caret = CARET.matcher(line);
        if (!pending.caretSeen && caret.matches()) {
            pending.caretSeen = true;
            if (pending.column == 0) {
                pending.column = column(caret.group(1));
            }
            return;
        }
        if (pending.caretSeen) {
            pending.details.add(line.strip());
        } else {
            pending.rest.add(line);
        }
    }

    private void flush() {
        if (pending == null) {
            return;
        }
        StringBuilder message = new StringBuilder(pending.message);
        if (!pending.caretSeen) {
            // No caret, so no source line to tell apart: an indented line after the header is
            // more message (a Zinc problem without a position, a javac note's details).
            for (String r : pending.rest) {
                if (!r.isBlank() && Character.isWhitespace(r.charAt(0))) {
                    message.append('\n').append(r.strip());
                }
            }
        } else if (pending.rest.size() > 1) {
            // Multi-line messages print their extra lines before the source line.
            for (String r : pending.rest.subList(0, pending.rest.size() - 1)) {
                message.append('\n').append(r.strip());
            }
        }
        for (String d : pending.details) {
            message.append('\n').append(d);
        }
        problems.add(new Problem(pending.file, pending.line, pending.column, pending.severity, message.toString()));
        pending = null;
    }

    /** The 1-based column of a caret after this indentation; a tab counts as one character, as javac's source echo keeps tabs. */
    private static int column(String indent) {
        return indent.length() + 1;
    }

    private static String severity(String word) {
        return switch (word.toLowerCase()) {
            case "error", "e" -> "error";
            case "warning", "warn", "w" -> "warning";
            default -> "info";
        };
    }

    private static String stripAnsi(String s) {
        return s.indexOf('\u001b') < 0 ? s : s.replaceAll("\u001b\\[[0-9;]*[A-Za-z]", "");
    }
}
