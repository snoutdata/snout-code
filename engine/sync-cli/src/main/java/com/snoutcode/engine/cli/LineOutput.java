package com.snoutcode.engine.cli;

import java.io.ByteArrayOutputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.function.Consumer;

/** An output stream that hands each complete line, UTF-8 decoded and without its line end, to a consumer. */
final class LineOutput extends OutputStream {
    private final Consumer<String> lines;
    private final ByteArrayOutputStream buffer = new ByteArrayOutputStream();

    LineOutput(Consumer<String> lines) {
        this.lines = lines;
    }

    @Override
    public synchronized void write(int b) {
        if (b == '\n') {
            emit();
        } else {
            buffer.write(b);
        }
    }

    @Override
    public synchronized void write(byte[] bytes, int off, int len) {
        int start = off;
        for (int i = off; i < off + len; i++) {
            if (bytes[i] == '\n') {
                buffer.write(bytes, start, i - start);
                emit();
                start = i + 1;
            }
        }
        buffer.write(bytes, start, off + len - start);
    }

    @Override
    public synchronized void close() {
        if (buffer.size() > 0) {
            emit();
        }
    }

    private void emit() {
        String line = buffer.toString(StandardCharsets.UTF_8);
        buffer.reset();
        if (line.endsWith("\r")) {
            line = line.substring(0, line.length() - 1);
        }
        lines.accept(line);
    }
}
