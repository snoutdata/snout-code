package com.snoutcode.engine.cli;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;

/**
 * A Gradle failure with what Gradle printed about it ("What went wrong"), for a build whose
 * Tooling API exception does not say: one that fails in a way the client cannot deserialize
 * reports only {@code invalid type code: 00}. The message is Gradle's text; the cause is the
 * Tooling API's exception.
 */
final class GradleSaid extends RuntimeException {
    private static final long serialVersionUID = 1L;

    /** How the message starts, so {@link ProjectSync#reason} can tell Gradle's words apart. */
    static final String PREFIX = "Gradle: ";

    GradleSaid(String said, Throwable cause) {
        super(PREFIX + said, cause);
    }

    /** The "What went wrong" part of Gradle's error output, without its markers; null if none. */
    static String whatWentWrong(String output) {
        int at = output.lastIndexOf("* What went wrong:");
        if (at < 0) {
            return null;
        }
        String rest = output.substring(at + "* What went wrong:".length());
        int end = rest.indexOf("\n* ");
        String text = (end < 0 ? rest : rest.substring(0, end)).strip();
        return text.isEmpty() ? null : text;
    }

    /** The most specific line of Gradle's account: its deepest {@code >} line, else its first. */
    static String deepest(String said) {
        String best = null;
        int depth = -1;
        for (String line : said.split("\n")) {
            String trimmed = line.strip();
            if (!trimmed.startsWith(">")) {
                continue;
            }
            int indent = line.indexOf('>');
            if (indent > depth) {
                depth = indent;
                best = trimmed.substring(1).strip();
            }
        }
        return best != null ? best : said.split("\n")[0].strip();
    }

    /** Passes output on (when there is somewhere to) and keeps its last 64 KB. */
    static final class Tail extends OutputStream {
        private static final int LIMIT = 64 * 1024;
        private final OutputStream next;
        private final ByteArrayOutputStream kept = new ByteArrayOutputStream();

        Tail(OutputStream next) {
            this.next = next;
        }

        @Override
        public synchronized void write(int b) throws IOException {
            write(new byte[] {(byte) b}, 0, 1);
        }

        @Override
        public synchronized void write(byte[] b, int off, int len) throws IOException {
            if (next != null) {
                next.write(b, off, len);
            }
            kept.write(b, off, len);
            if (kept.size() > 2 * LIMIT) {
                byte[] all = kept.toByteArray();
                kept.reset();
                kept.write(all, all.length - LIMIT, LIMIT);
            }
        }

        @Override
        public void flush() throws IOException {
            if (next != null) {
                next.flush();
            }
        }

        synchronized String text() {
            return kept.toString(StandardCharsets.UTF_8);
        }
    }
}
