package com.snoutcode.engine.lsp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.MethodOrderer;
import org.junit.jupiter.api.Order;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;
import org.junit.jupiter.api.TestMethodOrder;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Spring support through {@code snout-engine lsp}: the bean model, endpoints and injection
 * points of a small Spring Boot application, its missing and ambiguous beans as diagnostics with
 * quick fixes, {@code @Value} inlay hints, and completion, hover, definition and validation in
 * its application properties and YAML files.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
class LspSpringTest {
    private static final Duration LONG = Duration.ofMinutes(5);

    private Path project;
    private LspClient client;
    private Path controller;

    @BeforeAll
    void start() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("spring-" + System.nanoTime());
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.fixtures"), "spring-app"), scratch.resolve("spring-app"));
        project = scratch.resolve("spring-app");
        controller = project.resolve("src/main/java/demo/web/GreetingController.java");
        List<String> command = List.of(System.getProperty("snout.test.engine"), "lsp", project.toString(),
            "--cache-dir", scratch.resolve("cache").toString(), "--gradle-version", System.getProperty("snout.test.gradleVersion"),
            "--java-home", System.getProperty("java.home"), "--no-sources", "--no-debugger");
        client = LspClient.start(command, Map.of("JAVA_HOME", System.getProperty("java.home")), scratch.resolve("stderr.log").toFile());
        Driver driver = new Driver(project, client);
        driver.initialize(new JsonObject());
        driver.awaitReady(Duration.ofMinutes(10));
    }

    @AfterAll
    void stop() throws IOException {
        if (client != null) {
            client.close();
        }
    }

    private static JsonObject params(String json) {
        return JsonParser.parseString(json).getAsJsonObject();
    }

    @Test
    void readsTheBeans() throws IOException {
        JsonObject result = client.request("snout/springBeans", new JsonObject(), LONG).getAsJsonObject();
        JsonArray modules = result.getAsJsonArray("modules");
        assertEquals(1, modules.size(), result.toString());
        JsonObject module = modules.get(0).getAsJsonObject();
        assertEquals("spring-app", module.get("module").getAsString());
        List<String> names = new ArrayList<>();
        for (JsonElement b : module.getAsJsonArray("beans")) {
            names.add(b.getAsJsonObject().get("name").getAsString());
        }
        for (String expected : List.of("demoApplication", "demoProperties", "englishGreeting", "frenchGreeting", "greetingController",
            "statusController", "appConfig", "clock", "jsonSample")) {
            assertTrue(names.contains(expected), expected + " in " + names);
        }
        assertEquals("demo.DemoApplication", module.getAsJsonArray("applications").get(0).getAsJsonObject().get("class").getAsString());
        assertTrue(module.get("libraryBeans").getAsInt() > 100, module.get("libraryBeans").toString());
        assertTrue(result.toString().contains("\"kind\":\"restController\""), result.toString());
    }

    @Test
    void readsTheEndpoints() throws IOException {
        JsonArray endpoints = client.request("snout/springEndpoints", params("{\"module\":\"spring-app\"}"), LONG).getAsJsonObject()
            .getAsJsonArray("endpoints");
        String text = endpoints.toString();
        assertEquals(3, endpoints.size(), text);
        JsonObject echo = find(endpoints, "path", "/api/echo");
        assertEquals("[\"POST\"]", echo.get("methods").toString());
        assertEquals("[\"application/json\"]", echo.get("consumes").toString());
        assertEquals("echo", echo.getAsJsonObject("handler").get("method").getAsString());
        assertEquals("demo.web.GreetingController", echo.getAsJsonObject("handler").get("class").getAsString());
        assertEquals("[\"GET\"]", find(endpoints, "path", "/api/hello").get("methods").toString());
        JsonObject status = find(endpoints, "path", "/status");
        assertEquals("[\"/status\",\"/health\"]", status.get("paths").toString());
        assertEquals("[\"GET\",\"HEAD\"]", status.get("methods").toString());
    }

    @Test
    void resolvesInjectionPoints() throws IOException {
        JsonObject result = client.request("snout/springInjections", params("{\"uri\":\"" + LspClient.uri(controller) + "\"}"), LONG)
            .getAsJsonObject();
        List<JsonObject> injections = new ArrayList<>();
        for (JsonElement line : result.getAsJsonArray("lines")) {
            line.getAsJsonObject().getAsJsonArray("injections").forEach(i -> injections.add(i.getAsJsonObject()));
        }
        JsonObject greetings = injections.stream().filter(i -> "greetings".equals(name(i)) && "constructor".equals(i.get("kind").getAsString()))
            .findFirst().orElseThrow();
        assertEquals("ambiguous", greetings.get("status").getAsString(), greetings.toString());
        assertEquals(2, greetings.getAsJsonArray("beans").size(), greetings.toString());
        JsonObject audit = injections.stream().filter(i -> "audit".equals(name(i))).findFirst().orElseThrow();
        assertEquals("missing", audit.get("status").getAsString(), audit.toString());
        JsonObject clock = injections.stream().filter(i -> "clock".equals(name(i))).findFirst().orElseThrow();
        assertEquals("resolved", clock.get("status").getAsString(), clock.toString());
        assertEquals("clock", clock.getAsJsonArray("beans").get(0).getAsJsonObject().get("name").getAsString());
        JsonObject prefix = injections.stream().filter(i -> "prefix".equals(name(i))).findFirst().orElseThrow();
        assertEquals("value", prefix.get("status").getAsString(), prefix.toString());
        JsonObject properties = injections.stream().filter(i -> "properties".equals(name(i))).findFirst().orElseThrow();
        assertEquals("resolved", properties.get("status").getAsString(), properties.toString());

        // The bean side: EnglishGreeting knows where it goes.
        Path english = project.resolve("src/main/java/demo/greeting/EnglishGreeting.java");
        JsonObject bean = client.request("snout/springInjections", params("{\"path\":\"" + LspEndToEndTest.json(english.toString()) + "\"}"), LONG)
            .getAsJsonObject().getAsJsonArray("lines").get(0).getAsJsonObject().getAsJsonArray("beans").get(0).getAsJsonObject();
        assertEquals("englishGreeting", bean.get("name").getAsString());
        assertTrue(bean.getAsJsonArray("injectedInto").toString().contains("GreetingController"), bean.toString());

        // A @Bean method's parameter takes a library bean (Jackson's ObjectMapper).
        Path config = project.resolve("src/main/java/demo/config/AppConfig.java");
        String text = client.request("snout/springInjections", params("{\"uri\":\"" + LspClient.uri(config) + "\"}"), LONG).toString();
        assertTrue(text.contains("\"name\":\"mapper\"") && text.contains("\"kind\":\"library\""), text);
    }

    private static String name(JsonObject i) {
        return i.has("name") ? i.get("name").getAsString() : null;
    }

    @Test
    void warnsAboutMissingAndAmbiguousBeans() throws Exception {
        client.didOpen(controller);
        JsonArray diagnostics = awaitDiagnostics(controller, d -> d.toString().contains("spring.missingBean"));
        String text = diagnostics.toString();
        assertTrue(text.contains("No bean of type 'AuditLog' found"), text);
        assertTrue(text.contains("2 beans of type 'GreetingService' match: englishGreeting, frenchGreeting")
            || text.contains("2 beans of type 'GreetingService' match: frenchGreeting, englishGreeting"), text);
        long springWarnings = diagnostics.asList().stream().filter(d -> "spring".equals(d.getAsJsonObject().get("source").getAsString())).count();
        assertEquals(2, springWarnings, text);

        JsonObject ambiguous = diagnostics.asList().stream().map(JsonElement::getAsJsonObject)
            .filter(d -> "spring.ambiguousBean".equals(d.get("code").getAsString())).findFirst().orElseThrow();
        JsonObject request = new JsonObject();
        JsonObject document = new JsonObject();
        document.addProperty("uri", LspClient.uri(controller));
        request.add("textDocument", document);
        request.add("range", ambiguous.get("range"));
        JsonObject context = new JsonObject();
        JsonArray only = new JsonArray();
        only.add(ambiguous);
        context.add("diagnostics", only);
        request.add("context", context);
        JsonArray actions = client.request("textDocument/codeAction", request, LONG).getAsJsonArray();
        JsonObject fix = actions.asList().stream().map(JsonElement::getAsJsonObject)
            .filter(a -> a.has("title") && a.get("title").getAsString().equals("Inject 'englishGreeting' with @Qualifier")).findFirst()
            .orElseThrow(() -> new AssertionError(actions.toString()));
        String edits = fix.getAsJsonObject("edit").toString();
        assertTrue(edits.contains("@Qualifier(\\\"englishGreeting\\\") "), edits);
        assertTrue(edits.contains("import org.springframework.beans.factory.annotation.Qualifier;"), edits);

        JsonObject missing = diagnostics.asList().stream().map(JsonElement::getAsJsonObject)
            .filter(d -> "spring.missingBean".equals(d.get("code").getAsString())).findFirst().orElseThrow();
        request.add("range", missing.get("range"));
        JsonArray more = client.request("textDocument/codeAction", request, LONG).getAsJsonArray();
        assertTrue(more.toString().contains("@Autowired(required = false)"), more.toString());
    }

    @Test
    void showsValuePlaceholdersAsInlayHints() throws Exception {
        client.didOpen(controller);
        JsonObject request = new JsonObject();
        JsonObject document = new JsonObject();
        document.addProperty("uri", LspClient.uri(controller));
        request.add("textDocument", document);
        request.add("range", JsonParser.parseString("{\"start\":{\"line\":0,\"character\":0},\"end\":{\"line\":60,\"character\":0}}"));
        JsonElement hints = client.request("textDocument/inlayHint", request, LONG);
        String text = hints.toString();
        assertTrue(text.contains("= Howdy"), text);
        assertTrue(text.contains("= none (default)"), text);
    }

    @Test
    void completesAndChecksApplicationProperties() throws Exception {
        Path file = project.resolve("src/main/resources/application.properties");
        open(file, "properties");
        JsonArray diagnostics = awaitDiagnostics(file, d -> d.size() >= 2);
        String text = diagnostics.toString();
        assertEquals(2, diagnostics.size(), text);
        assertTrue(text.contains("Unknown property 'demo.unknown-key'"), text);
        assertTrue(text.contains("'often' is not a whole number"), text);

        // Keys: Spring Boot's and the application's own.
        JsonObject completion = client.request("textDocument/completion", LspClient.position(file, 0, 9), LONG).getAsJsonObject();
        assertTrue(labels(completion).contains("server.port"), labels(completion).toString());
        JsonObject own = client.request("textDocument/completion", LspClient.position(file, 2, 5), LONG).getAsJsonObject();
        assertTrue(labels(own).containsAll(List.of("demo.name", "demo.retries", "demo.timeout", "demo.mode", "demo.greeting.prefix")),
            labels(own).toString());
        // Values: an enum's constants.
        JsonObject values = client.request("textDocument/completion", LspClient.position(file, 4, 10), LONG).getAsJsonObject();
        assertTrue(labels(values).containsAll(List.of("careful", "fast")), labels(values).toString());
        JsonObject levels = client.request("textDocument/completion", LspClient.position(file, 7, 19), LONG).getAsJsonObject();
        assertTrue(labels(levels).contains("debug"), labels(levels).toString());

        JsonObject hover = client.request("textDocument/hover", LspClient.position(file, 0, 3), LONG).getAsJsonObject();
        assertTrue(hover.toString().contains("server.port") && hover.toString().contains("Integer"), hover.toString());
        JsonObject ownHover = client.request("textDocument/hover", LspClient.position(file, 2, 3), LONG).getAsJsonObject();
        assertTrue(ownHover.toString().contains("The name the demo greets."), ownHover.toString());

        JsonArray definition = client.request("textDocument/definition", LspClient.position(file, 2, 3), LONG).getAsJsonArray();
        assertTrue(definition.toString().contains("DemoProperties.java"), definition.toString());
        JsonArray boot = client.request("textDocument/definition", LspClient.position(file, 0, 3), LONG).getAsJsonArray();
        assertFalse(boot.isEmpty(), boot.toString());

        // Fixing the value clears its warning.
        JsonObject change = params("""
            { "textDocument": { "uri": "%s", "version": 2 },
              "contentChanges": [ { "range": { "start": { "line": 3, "character": 13 }, "end": { "line": 3, "character": 18 } }, "text": "5" } ] }"""
            .formatted(LspClient.uri(file)));
        client.notify("textDocument/didChange", change);
        JsonArray after = awaitDiagnostics(file, d -> d.size() == 1);
        assertEquals(1, after.size(), after.toString());
    }

    @Test
    void completesAndChecksApplicationYaml() throws Exception {
        Path file = project.resolve("src/main/resources/application-dev.yml");
        open(file, "yaml");
        JsonArray diagnostics = awaitDiagnostics(file, d -> !d.isEmpty());
        assertEquals(1, diagnostics.size(), diagnostics.toString());
        assertTrue(diagnostics.toString().contains("Unknown property 'demo.bogus'"), diagnostics.toString());
        JsonObject completion = client.request("textDocument/completion", LspClient.position(file, 3, 4), LONG).getAsJsonObject();
        assertTrue(labels(completion).contains("demo.timeout"), labels(completion).toString());
        String inserted = completion.toString();
        assertTrue(inserted.contains("\"newText\":\"timeout: \""), inserted);
        JsonObject hover = client.request("textDocument/hover", LspClient.position(file, 1, 3), LONG).getAsJsonObject();
        assertTrue(hover.toString().contains("server.port"), hover.toString());
    }

    @Test
    @Order(Integer.MAX_VALUE - 1)
    void evaluatesTheChosenProfiles() throws Exception {
        JsonObject profiles = client.request("snout/springProfiles", new JsonObject(), LONG).getAsJsonObject();
        assertEquals("any", profiles.get("mode").getAsString(), profiles.toString());
        JsonObject module = profiles.getAsJsonArray("modules").get(0).getAsJsonObject();
        assertFalse(module.has("active"), module.toString());
        assertTrue(module.getAsJsonArray("known").toString().contains("{\"name\":\"dev\",\"sources\":[\"application-dev.yml\"]}"),
            module.toString());

        Path dev = project.resolve("src/main/resources/application-dev.yml");
        open(dev, "yaml");
        JsonObject set = client.request("snout/springSetProfiles", params("{\"mode\":\"chosen\",\"profiles\":[\"prod\"]}"), LONG)
            .getAsJsonObject();
        assertEquals("chosen", set.get("mode").getAsString(), set.toString());
        assertEquals("[\"prod\"]", set.getAsJsonArray("modules").get(0).getAsJsonObject().get("active").toString(), set.toString());
        JsonArray inactive = awaitDiagnostics(dev, d -> d.toString().contains("spring.inactiveProfile"));
        assertTrue(inactive.toString().contains("profile 'dev' is not active (active: prod)"), inactive.toString());
        // The beans: none has a profile here, so each stays active.
        JsonObject beans = client.request("snout/springBeans", new JsonObject(), LONG).getAsJsonObject();
        assertFalse(beans.toString().contains("\"active\":false"), beans.toString());
        assertTrue(beans.toString().contains("\"activeProfiles\":[\"prod\"]"), beans.toString());

        client.request("snout/springSetProfiles", params("{\"mode\":\"any\"}"), LONG);
        JsonArray back = awaitDiagnostics(dev, d -> !d.toString().contains("spring.inactiveProfile"));
        assertFalse(back.toString().contains("spring.inactiveProfile"), back.toString());
    }

    @Test
    @Order(Integer.MAX_VALUE)
    void followsEditsOfJavaFiles() throws Exception {
        client.didOpen(controller);
        awaitDiagnostics(controller, d -> d.toString().contains("spring.ambiguousBean"));
        String text = Files.readString(controller).replace("public GreetingController(GreetingService greetings",
            "public GreetingController(@org.springframework.beans.factory.annotation.Qualifier(\"frenchGreeting\") GreetingService greetings");
        JsonObject change = new JsonObject();
        JsonObject document = new JsonObject();
        document.addProperty("uri", LspClient.uri(controller));
        document.addProperty("version", 2);
        change.add("textDocument", document);
        JsonArray changes = new JsonArray();
        JsonObject whole = new JsonObject();
        whole.addProperty("text", text);
        changes.add(whole);
        change.add("contentChanges", changes);
        client.notify("textDocument/didChange", change);
        JsonArray after = awaitDiagnostics(controller, d -> !d.toString().contains("spring.ambiguousBean"));
        assertFalse(after.toString().contains("spring.ambiguousBean"), after.toString());
        assertTrue(after.toString().contains("spring.missingBean"), after.toString());
        JsonObject result = client.request("snout/springInjections", params("{\"uri\":\"" + LspClient.uri(controller) + "\"}"), LONG)
            .getAsJsonObject();
        assertTrue(result.toString().contains("\"qualifiers\":[\"frenchGreeting\"]"), result.toString());
    }

    private void open(Path file, String language) throws IOException {
        JsonObject document = new JsonObject();
        document.addProperty("uri", LspClient.uri(file));
        document.addProperty("languageId", language);
        document.addProperty("version", 1);
        document.addProperty("text", Files.readString(file));
        JsonObject p = new JsonObject();
        p.add("textDocument", document);
        client.notify("textDocument/didOpen", p);
    }

    private JsonArray awaitDiagnostics(Path file, java.util.function.Predicate<JsonArray> until) throws InterruptedException {
        long deadline = System.nanoTime() + LONG.toNanos();
        JsonArray last = null;
        while (System.nanoTime() < deadline) {
            last = client.diagnostics.get(LspClient.uri(file));
            if (last != null && until.test(last)) {
                return last;
            }
            Thread.sleep(200);
        }
        assertNotNull(last, "no diagnostics for " + file);
        return last;
    }

    private static List<String> labels(JsonObject completion) {
        List<String> out = new ArrayList<>();
        for (JsonElement i : completion.getAsJsonArray("items")) {
            out.add(i.getAsJsonObject().get("label").getAsString());
        }
        return out;
    }

    private static JsonObject find(JsonArray array, String key, String value) {
        for (JsonElement e : array) {
            if (e.getAsJsonObject().has(key) && value.equals(e.getAsJsonObject().get(key).getAsString())) {
                return e.getAsJsonObject();
            }
        }
        throw new AssertionError("no " + key + "=" + value + " in " + array);
    }
}
