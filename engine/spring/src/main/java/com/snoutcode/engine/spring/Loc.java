package com.snoutcode.engine.spring;

import com.google.gson.JsonObject;

/** A range in a file, as LSP has it: 0-based lines, columns in UTF-16 code units. */
final class Loc {
    final String uri;
    final int line;
    final int character;
    final int endLine;
    final int endCharacter;

    Loc(String uri, int line, int character, int endLine, int endCharacter) {
        this.uri = uri;
        this.line = line;
        this.character = character;
        this.endLine = endLine;
        this.endCharacter = endCharacter;
    }

    /** The range of {@code length} characters from {@code offset} in {@code text}. */
    static Loc of(String uri, LineIndex lines, int offset, int length) {
        int[] start = lines.position(offset);
        int[] end = lines.position(offset + length);
        return new Loc(uri, start[0], start[1], end[0], end[1]);
    }

    JsonObject range() {
        return range(line, character, endLine, endCharacter);
    }

    static JsonObject range(int line, int character, int endLine, int endCharacter) {
        JsonObject range = new JsonObject();
        range.add("start", position(line, character));
        range.add("end", position(endLine, endCharacter));
        return range;
    }

    static JsonObject position(int line, int character) {
        JsonObject p = new JsonObject();
        p.addProperty("line", line);
        p.addProperty("character", character);
        return p;
    }

    JsonObject location() {
        JsonObject o = new JsonObject();
        o.addProperty("uri", uri);
        o.add("range", range());
        return o;
    }

    boolean contains(int l, int c) {
        if (l < line || l > endLine) {
            return false;
        }
        return !(l == line && c < character) && !(l == endLine && c > endCharacter);
    }

    @Override
    public String toString() {
        return uri + ":" + (line + 1) + ":" + (character + 1);
    }
}
