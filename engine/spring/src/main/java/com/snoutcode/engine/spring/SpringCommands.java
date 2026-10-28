package com.snoutcode.engine.spring;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.eclipse.core.runtime.IProgressMonitor;
import org.eclipse.jdt.ls.core.internal.IDelegateCommandHandler;

import java.util.ArrayList;
import java.util.List;

/**
 * The commands snout-engine sends through {@code workspace/executeCommand}. Each takes one
 * argument, a JSON object (or its text), and answers JSON:
 *
 * <ul>
 *   <li>{@code snout.spring.scan {}}: reads the whole workspace; {@code {diagnostics: {uri: [Diagnostic]}, stats}}.</li>
 *   <li>{@code snout.spring.update {uris}}: reads these files again; {@code {diagnostics}} of the files whose
 *       Spring diagnostics changed.</li>
 *   <li>{@code snout.spring.beans {module?}}, {@code snout.spring.endpoints {module?}},
 *       {@code snout.spring.injections {uri}}: the model, as {@code snout/springBeans} and the others answer.</li>
 *   <li>{@code snout.spring.properties {action, uri, text, line?, character?}}: {@code complete},
 *       {@code hover}, {@code definition} or {@code validate} in an application properties or YAML file.</li>
 *   <li>{@code snout.spring.inlayHints {uri, range}}: the values of {@code @Value} placeholders.</li>
 * </ul>
 */
public final class SpringCommands implements IDelegateCommandHandler {
    private static final Gson GSON = new Gson();

    @Override
    public Object executeCommand(String commandId, List<Object> arguments, IProgressMonitor monitor) throws Exception {
        JsonObject params = argument(arguments);
        SpringWorkspace w = SpringWorkspace.INSTANCE;
        return switch (commandId) {
            case "snout.spring.scan" -> w.scan(monitor);
            case "snout.spring.update" -> w.update(strings(params, "uris"), monitor);
            case "snout.spring.beans" -> w.beans(string(params, "module"), monitor);
            case "snout.spring.endpoints" -> w.endpoints(string(params, "module"), monitor);
            case "snout.spring.injections" -> w.injections(string(params, "uri"), monitor);
            case "snout.spring.properties" -> properties(params, monitor);
            case "snout.spring.inlayHints" -> {
                JsonObject range = params.has("range") ? params.getAsJsonObject("range") : null;
                int from = range == null ? 0 : range.getAsJsonObject("start").get("line").getAsInt();
                int to = range == null ? Integer.MAX_VALUE : range.getAsJsonObject("end").get("line").getAsInt();
                yield w.inlayHints(string(params, "uri"), from, to, monitor);
            }
            default -> throw new UnsupportedOperationException(commandId);
        };
    }

    private static JsonElement properties(JsonObject params, IProgressMonitor monitor) throws Exception {
        String uri = string(params, "uri");
        String text = string(params, "text");
        if (uri == null || text == null) {
            throw new IllegalArgumentException("properties needs uri and text");
        }
        PropsDoc doc = PropsDoc.parse(text, PropsDoc.isYaml(uri));
        SpringWorkspace w = SpringWorkspace.INSTANCE;
        PropertiesService service = w.properties(uri, text, monitor);
        int line = params.has("line") ? params.get("line").getAsInt() : 0;
        int character = params.has("character") ? params.get("character").getAsInt() : 0;
        String action = string(params, "action");
        JsonElement out = switch (action == null ? "" : action) {
            case "complete" -> service.complete(doc, line, character);
            case "hover" -> service.hover(doc, line, character);
            case "validate" -> service.validate(doc);
            case "definition" -> {
                PropsDoc.Entry e = service.keyAt(doc, line, character);
                yield e == null ? new com.google.gson.JsonArray() : w.definition(uri, service, e.key);
            }
            default -> throw new IllegalArgumentException("unknown properties action " + action);
        };
        return out == null ? JsonNull.INSTANCE : out;
    }

    private static JsonObject argument(List<Object> arguments) {
        if (arguments == null || arguments.isEmpty() || arguments.get(0) == null) {
            return new JsonObject();
        }
        Object a = arguments.get(0);
        JsonElement e;
        if (a instanceof JsonElement j) {
            e = j;
        } else {
            e = GSON.toJsonTree(a);
        }
        if (e.isJsonPrimitive() && e.getAsJsonPrimitive().isString()) {
            e = JsonParser.parseString(e.getAsString());
        }
        return e.isJsonObject() ? e.getAsJsonObject() : new JsonObject();
    }

    private static String string(JsonObject o, String key) {
        JsonElement e = o.get(key);
        return e == null || e.isJsonNull() ? null : e.getAsString();
    }

    private static List<String> strings(JsonObject o, String key) {
        List<String> out = new ArrayList<>();
        JsonElement e = o.get(key);
        if (e != null && e.isJsonArray()) {
            e.getAsJsonArray().forEach(x -> out.add(x.getAsString()));
        }
        return out;
    }
}
