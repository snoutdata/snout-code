package com.snoutcode.engine.junit;

import org.junit.platform.engine.TestExecutionResult;
import org.junit.platform.engine.TestSource;
import org.junit.platform.engine.support.descriptor.ClassSource;
import org.junit.platform.engine.support.descriptor.MethodSource;
import org.junit.platform.launcher.TestExecutionListener;
import org.junit.platform.launcher.TestIdentifier;
import org.junit.platform.launcher.TestPlan;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintStream;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.io.Writer;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * Sends a test run's events to SnoutCode: suites and tests started and finished (with the
 * outcome, duration, failure message and stack trace) and what each test prints, as JSON lines
 * to the address in {@code SNOUT_TEST_EVENTS} ({@code host:port}). Without that variable it
 * does nothing. The engines' own roots are left out: their children are the run's top level.
 */
public final class SnoutTestEvents implements TestExecutionListener {
    private Writer out;
    private TestPlan plan;
    private final Set<String> shown = new HashSet<>();
    private final Map<String, Long> started = new HashMap<>();
    private volatile String current;
    private PrintStream originalOut;
    private PrintStream originalErr;

    @Override
    public void testPlanExecutionStarted(TestPlan testPlan) {
        String address = System.getenv("SNOUT_TEST_EVENTS");
        if (address == null || address.isEmpty()) {
            return;
        }
        int colon = address.lastIndexOf(':');
        try {
            @SuppressWarnings("resource")
            Socket socket = new Socket(address.substring(0, colon), Integer.parseInt(address.substring(colon + 1)));
            out = new OutputStreamWriter(socket.getOutputStream(), StandardCharsets.UTF_8);
        } catch (IOException | RuntimeException e) {
            out = null;
            return;
        }
        plan = testPlan;
        originalOut = System.out;
        originalErr = System.err;
        System.setOut(new PrintStream(new Tee(originalOut, "stdout"), true));
        System.setErr(new PrintStream(new Tee(originalErr, "stderr"), true));
    }

    @Override
    public void testPlanExecutionFinished(TestPlan testPlan) {
        if (out == null) {
            return;
        }
        System.out.flush();
        System.err.flush();
        System.setOut(originalOut);
        System.setErr(originalErr);
        try {
            out.close();
        } catch (IOException e) {
            // the engine is gone
        }
        out = null;
    }

    @Override
    public void executionStarted(TestIdentifier id) {
        if (out == null || !id.getParentId().isPresent()) {
            return;
        }
        shown.add(id.getUniqueId());
        started.put(id.getUniqueId(), System.currentTimeMillis());
        if (id.isTest()) {
            current = id.getUniqueId();
        }
        send(describe("started", id));
    }

    @Override
    public void executionSkipped(TestIdentifier id, String reason) {
        if (out == null || !id.getParentId().isPresent()) {
            return;
        }
        shown.add(id.getUniqueId());
        send(describe("started", id));
        Map<String, Object> e = describe("finished", id);
        e.put("outcome", "skipped");
        e.put("durationMillis", 0);
        if (reason != null) {
            e.put("message", reason);
        }
        send(e);
    }

    @Override
    public void executionFinished(TestIdentifier id, TestExecutionResult result) {
        if (out == null || !shown.contains(id.getUniqueId())) {
            return;
        }
        if (id.getUniqueId().equals(current)) {
            System.out.flush();
            System.err.flush();
            current = null;
        }
        Map<String, Object> e = describe("finished", id);
        TestExecutionResult.Status status = result.getStatus();
        e.put("outcome", status == TestExecutionResult.Status.SUCCESSFUL ? "passed"
            : status == TestExecutionResult.Status.FAILED ? "failed" : "skipped");
        Long start = started.remove(id.getUniqueId());
        e.put("durationMillis", start == null ? 0 : System.currentTimeMillis() - start);
        if (result.getThrowable().isPresent()) {
            Throwable t = result.getThrowable().get();
            e.put("message", t.getMessage() != null ? t.getMessage() : t.toString());
            e.put("exception", t.getClass().getName());
            StringWriter trace = new StringWriter();
            t.printStackTrace(new PrintWriter(trace));
            e.put("stackTrace", trace.toString());
        }
        send(e);
    }

    private Map<String, Object> describe(String event, TestIdentifier id) {
        Map<String, Object> e = new LinkedHashMap<>();
        e.put("event", event);
        e.put("kind", id.isTest() ? "test" : "suite");
        e.put("id", id.getUniqueId());
        String parent = id.getParentId().orElse(null);
        e.put("parent", parent != null && shown.contains(parent) ? parent : null);
        String className = null;
        String name = id.getDisplayName();
        TestSource source = id.getSource().orElse(null);
        if (source instanceof MethodSource) {
            MethodSource m = (MethodSource) source;
            className = m.getClassName();
            name = m.getMethodName() + "(" + (m.getMethodParameterTypes() == null ? "" : m.getMethodParameterTypes()) + ")";
        } else if (source instanceof ClassSource) {
            className = ((ClassSource) source).getClassName();
            name = className;
        }
        e.put("name", name);
        e.put("displayName", id.getDisplayName());
        e.put("className", className);
        e.put("task", "surefire");
        return e;
    }

    private synchronized void send(Map<String, Object> event) {
        if (out == null) {
            return;
        }
        try {
            out.write(json(event));
            out.write('\n');
            out.flush();
        } catch (IOException e) {
            out = null;
        }
    }

    static String json(Map<String, Object> map) {
        StringBuilder s = new StringBuilder("{");
        for (Map.Entry<String, Object> e : map.entrySet()) {
            if (s.length() > 1) {
                s.append(',');
            }
            quote(s, e.getKey());
            s.append(':');
            Object v = e.getValue();
            if (v == null) {
                s.append("null");
            } else if (v instanceof Number || v instanceof Boolean) {
                s.append(v);
            } else {
                quote(s, v.toString());
            }
        }
        return s.append('}').toString();
    }

    private static void quote(StringBuilder s, String text) {
        s.append('"');
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            switch (c) {
                case '"': s.append("\\\""); break;
                case '\\': s.append("\\\\"); break;
                case '\n': s.append("\\n"); break;
                case '\r': s.append("\\r"); break;
                case '\t': s.append("\\t"); break;
                default:
                    if (c < 0x20) {
                        s.append(String.format("\\u%04x", (int) c));
                    } else {
                        s.append(c);
                    }
            }
        }
        s.append('"');
    }

    /** Passes output on, and sends what a running test prints as its output, a line at a time. */
    private final class Tee extends OutputStream {
        private final PrintStream to;
        private final String destination;
        private final ByteArrayOutputStream line = new ByteArrayOutputStream();

        Tee(PrintStream to, String destination) {
            this.to = to;
            this.destination = destination;
        }

        @Override
        public synchronized void write(int b) {
            to.write(b);
            line.write(b);
            if (b == '\n') {
                flushLine();
            }
        }

        @Override
        public synchronized void write(byte[] b, int off, int len) {
            for (int i = off; i < off + len; i++) {
                write(b[i]);
            }
        }

        @Override
        public synchronized void flush() {
            to.flush();
            flushLine();
        }

        private void flushLine() {
            String test = current;
            if (line.size() == 0) {
                return;
            }
            String text = new String(line.toByteArray(), StandardCharsets.UTF_8);
            line.reset();
            if (test == null) {
                return;
            }
            Map<String, Object> e = new LinkedHashMap<>();
            e.put("event", "output");
            e.put("kind", "test");
            e.put("id", test);
            e.put("destination", destination);
            e.put("text", text);
            send(e);
        }
    }
}
