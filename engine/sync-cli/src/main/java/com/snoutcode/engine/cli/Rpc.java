package com.snoutcode.engine.cli;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.ByteArrayOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;

/** JSON-RPC framing as LSP and DAP use it: {@code Content-Length} headers, a blank line, the body. */
final class Rpc {
    private Rpc() {
    }

    /** Reads one message; throws {@link EOFException} at the end of the stream. */
    static JsonObject read(InputStream in) throws IOException {
        while (true) {
            int length = -1;
            String header;
            while (!(header = readLine(in)).isEmpty()) {
                int colon = header.indexOf(':');
                if (colon > 0 && header.substring(0, colon).trim().equalsIgnoreCase("content-length")) {
                    length = Integer.parseInt(header.substring(colon + 1).trim());
                }
            }
            if (length < 0) {
                continue;
            }
            byte[] body = in.readNBytes(length);
            if (body.length < length) {
                throw new EOFException();
            }
            return JsonParser.parseString(new String(body, StandardCharsets.UTF_8)).getAsJsonObject();
        }
    }

    /** Writes one message. Callers serialize writes to the same stream. */
    static void write(OutputStream out, JsonElement message) throws IOException {
        byte[] body = message.toString().getBytes(StandardCharsets.UTF_8);
        out.write(("Content-Length: " + body.length + "\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
        out.write(body);
        out.flush();
    }

    static JsonObject notification(String method, JsonElement params) {
        JsonObject message = new JsonObject();
        message.addProperty("jsonrpc", "2.0");
        message.addProperty("method", method);
        message.add("params", params);
        return message;
    }

    static JsonObject request(JsonElement id, String method, JsonElement params) {
        JsonObject message = notification(method, params);
        message.add("id", id);
        return message;
    }

    static JsonObject result(JsonElement id, JsonElement result) {
        JsonObject message = new JsonObject();
        message.addProperty("jsonrpc", "2.0");
        message.add("id", id);
        message.add("result", result);
        return message;
    }

    static JsonObject error(JsonElement id, int code, String text) {
        JsonObject message = new JsonObject();
        message.addProperty("jsonrpc", "2.0");
        message.add("id", id);
        JsonObject error = new JsonObject();
        error.addProperty("code", code);
        error.addProperty("message", text);
        message.add("error", error);
        return message;
    }

    private static String readLine(InputStream in) throws IOException {
        ByteArrayOutputStream line = new ByteArrayOutputStream();
        int c;
        while ((c = in.read()) != -1) {
            if (c == '\n') {
                break;
            }
            if (c != '\r') {
                line.write(c);
            }
        }
        if (c == -1) {
            throw new EOFException();
        }
        return line.toString(StandardCharsets.US_ASCII);
    }
}
