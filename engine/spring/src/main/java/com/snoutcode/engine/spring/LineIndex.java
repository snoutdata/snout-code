package com.snoutcode.engine.spring;

import java.util.Arrays;

/** Turns offsets in a text into LSP positions and back. */
final class LineIndex {
    private final int[] starts;
    private final int length;

    LineIndex(String text) {
        int[] s = new int[16];
        int n = 0;
        s[n++] = 0;
        for (int i = 0; i < text.length(); i++) {
            if (text.charAt(i) == '\n') {
                if (n == s.length) {
                    s = Arrays.copyOf(s, n * 2);
                }
                s[n++] = i + 1;
            }
        }
        starts = Arrays.copyOf(s, n);
        length = text.length();
    }

    /** {line, character} of an offset. */
    int[] position(int offset) {
        int o = Math.max(0, Math.min(offset, length));
        int line = Arrays.binarySearch(starts, o);
        if (line < 0) {
            line = -line - 2;
        }
        return new int[] {line, o - starts[line]};
    }

    int offset(int line, int character) {
        if (line < 0) {
            return 0;
        }
        if (line >= starts.length) {
            return length;
        }
        return Math.min(starts[line] + Math.max(0, character), line + 1 < starts.length ? starts[line + 1] : length);
    }

    int lineCount() {
        return starts.length;
    }

    int lineStart(int line) {
        return starts[Math.max(0, Math.min(line, starts.length - 1))];
    }

    int lineEnd(String text, int line) {
        int end = line + 1 < starts.length ? starts[line + 1] - 1 : length;
        if (end > starts[line] && end <= text.length() && end > 0 && text.charAt(end - 1) == '\r') {
            end--;
        }
        return end;
    }
}
