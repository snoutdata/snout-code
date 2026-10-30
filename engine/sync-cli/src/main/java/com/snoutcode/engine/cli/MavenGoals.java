package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import org.eclipse.aether.RepositorySystem;
import org.eclipse.aether.RepositorySystemSession;
import org.eclipse.aether.artifact.DefaultArtifact;
import org.eclipse.aether.repository.RemoteRepository;
import org.eclipse.aether.resolution.ArtifactRequest;
import org.eclipse.aether.resolution.ArtifactResult;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

import javax.xml.parsers.DocumentBuilderFactory;
import java.io.File;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.concurrent.ConcurrentHashMap;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * What the Maven window lists for a build ({@code snout/mavenGoals}): per module, the lifecycle
 * phases, the plugins (those the POM declares and those the packaging binds by default) with
 * the goals their descriptors name, and the profiles. A plugin's goals come from its jar's
 * {@code META-INF/maven/plugin.xml}, resolved once (from the local repository when it is there).
 */
final class MavenGoals {
    /** The phases the window offers, in lifecycle order: clean's, then the default lifecycle's. */
    static final List<String> PHASES = List.of("clean", "validate", "compile", "test", "package", "verify", "install", "site", "deploy");

    /** The plugins Maven 3.9 binds to a {@code jar} module's lifecycle, with their default versions. */
    private static final String[][] JAR_BINDINGS = {
        {"org.apache.maven.plugins", "maven-clean-plugin", "3.2.0"},
        {"org.apache.maven.plugins", "maven-resources-plugin", "3.3.1"},
        {"org.apache.maven.plugins", "maven-compiler-plugin", "3.13.0"},
        {"org.apache.maven.plugins", "maven-surefire-plugin", "3.2.5"},
        {"org.apache.maven.plugins", "maven-jar-plugin", "3.4.1"},
        {"org.apache.maven.plugins", "maven-install-plugin", "3.1.2"},
        {"org.apache.maven.plugins", "maven-deploy-plugin", "3.1.2"},
        {"org.apache.maven.plugins", "maven-site-plugin", "3.12.1"},
    };

    private static final Map<String, JsonObject> DESCRIPTORS = new ConcurrentHashMap<>();

    private MavenGoals() {
    }

    static JsonObject of(ProjectSync.Plan plan, List<Modules.Ref> all) {
        long start = System.nanoTime();
        JsonObject out = new JsonObject();
        out.addProperty("build", plan.id());
        out.addProperty("rootDir", plan.rootDir().getAbsolutePath());
        out.addProperty("mavenVersion", MavenHome.wrapperVersion(plan.rootDir()));
        out.addProperty("wrapper", MavenHome.wrapper(plan.rootDir()) != null);
        JsonArray modules = new JsonArray();
        JsonArray profiles = new JsonArray();
        List<String> profileIds = new ArrayList<>();
        try {
            MavenEnv env = MavenEnv.load(plan.options().offline);
            RepositorySystemSession session = env.session(new Properties());
            RepositorySystem system = MavenEnv.system();
            List<RemoteRepository> repos = system.newResolutionRepositories(session, List.of(MavenSync.central()));
            for (Modules.Ref m : all) {
                if (!m.buildId().equals(plan.id())) {
                    continue;
                }
                JsonObject maven = Engine.object(m.project(), "maven");
                if (maven == null) {
                    continue;
                }
                JsonObject module = new JsonObject();
                module.addProperty("module", m.id());
                module.addProperty("project", m.gradlePath());
                module.addProperty("name", Engine.string(maven, "artifactId"));
                module.addProperty("displayName", Engine.string(maven, "name"));
                module.addProperty("packaging", Engine.string(maven, "packaging"));
                module.addProperty("projectDir", m.projectDir().toString());
                module.addProperty("buildFile", Engine.string(m.project(), "buildFile"));
                JsonArray phases = new JsonArray();
                PHASES.forEach(phases::add);
                module.add("lifecycle", phases);
                module.add("plugins", plugins(maven, session, system, repos));
                modules.add(module);
                JsonArray ps = Engine.array(maven, "profiles");
                if (ps != null) {
                    for (JsonElement p : ps) {
                        String id = Engine.string(p.getAsJsonObject(), "id");
                        if (!profileIds.contains(id)) {
                            profileIds.add(id);
                            profiles.add(p.deepCopy());
                        }
                    }
                }
            }
        } catch (RuntimeException e) {
            out.addProperty("error", SnoutEngine.describe(e));
        }
        out.add("modules", modules);
        out.add("profiles", profiles);
        out.addProperty("millis", (System.nanoTime() - start) / 1_000_000);
        return out;
    }

    private static JsonArray plugins(JsonObject maven, RepositorySystemSession session, RepositorySystem system, List<RemoteRepository> repos) {
        Map<String, String[]> plugins = new LinkedHashMap<>();
        String packaging = String.valueOf(Engine.string(maven, "packaging"));
        JsonObject managed = Engine.object(maven, "managedPlugins");
        if (!"pom".equals(packaging)) {
            for (String[] b : JAR_BINDINGS) {
                String[] p = b.clone();
                if ("war".equals(packaging) && p[1].equals("maven-jar-plugin")) {
                    p = new String[] {p[0], "maven-war-plugin", "3.4.0"};
                }
                String managedVersion = Engine.string(managed, p[0] + ":" + p[1]);
                if (managedVersion != null) {
                    p[2] = managedVersion;
                }
                plugins.put(p[0] + ":" + p[1], p);
            }
        } else {
            for (String[] b : JAR_BINDINGS) {
                if (b[1].equals("maven-clean-plugin") || b[1].equals("maven-install-plugin") || b[1].equals("maven-deploy-plugin")
                    || b[1].equals("maven-site-plugin")) {
                    plugins.put(b[0] + ":" + b[1], b.clone());
                }
            }
        }
        JsonArray declared = Engine.array(maven, "plugins");
        if (declared != null) {
            for (JsonElement e : declared) {
                JsonObject p = e.getAsJsonObject();
                String g = Engine.string(p, "groupId");
                String a = Engine.string(p, "artifactId");
                String v = Engine.string(p, "version");
                String[] existing = plugins.get(g + ":" + a);
                plugins.put(g + ":" + a, new String[] {g, a, v != null ? v : existing == null ? null : existing[2]});
            }
        }
        JsonArray out = new JsonArray();
        for (String[] p : plugins.values()) {
            JsonObject descriptor = p[2] == null ? null : descriptor(p[0], p[1], p[2], session, system, repos);
            JsonObject o = new JsonObject();
            o.addProperty("groupId", p[0]);
            o.addProperty("artifactId", p[1]);
            o.addProperty("version", p[2]);
            String prefix = descriptor == null ? null : Engine.string(descriptor, "prefix");
            o.addProperty("prefix", prefix != null ? prefix : MavenSync.prefix(p[1]));
            o.add("goals", descriptor == null ? new JsonArray() : descriptor.getAsJsonArray("goals"));
            out.add(o);
        }
        return out;
    }

    /** A plugin's goal prefix and goals (name and description) from its descriptor; null when it cannot be read. */
    static JsonObject descriptor(String g, String a, String v, RepositorySystemSession session, RepositorySystem system,
                                 List<RemoteRepository> repos) {
        String key = g + ":" + a + ":" + v;
        JsonObject cached = DESCRIPTORS.get(key);
        if (cached != null) {
            return cached;
        }
        try {
            ArtifactResult r = system.resolveArtifact(session, new ArtifactRequest(new DefaultArtifact(g, a, "", "jar", v), repos, null));
            JsonObject d = read(r.getArtifact().getFile());
            if (d != null) {
                DESCRIPTORS.put(key, d);
            }
            return d;
        } catch (Exception e) {
            return null;
        }
    }

    static JsonObject read(File jar) throws Exception {
        try (ZipFile zip = new ZipFile(jar)) {
            ZipEntry entry = zip.getEntry("META-INF/maven/plugin.xml");
            if (entry == null) {
                return null;
            }
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
            Document doc;
            try (InputStream in = zip.getInputStream(entry)) {
                doc = factory.newDocumentBuilder().parse(in);
            }
            Element root = doc.getDocumentElement();
            JsonObject out = new JsonObject();
            out.addProperty("prefix", childText(root, "goalPrefix"));
            JsonArray goals = new JsonArray();
            Element mojos = child(root, "mojos");
            if (mojos != null) {
                NodeList list = mojos.getChildNodes();
                for (int i = 0; i < list.getLength(); i++) {
                    if (list.item(i) instanceof Element mojo && "mojo".equals(mojo.getTagName())) {
                        String goal = childText(mojo, "goal");
                        if (goal == null || goal.equals("help")) {
                            continue;
                        }
                        JsonObject o = new JsonObject();
                        o.addProperty("name", goal);
                        String description = childText(mojo, "description");
                        o.addProperty("description", description == null ? null : firstSentence(description));
                        o.addProperty("phase", childText(mojo, "phase"));
                        goals.add(o);
                    }
                }
            }
            out.add("goals", goals);
            return out;
        }
    }

    private static String firstSentence(String html) {
        String text = html.replaceAll("<[^>]+>", "").replaceAll("&lt;", "<").replaceAll("&gt;", ">").replaceAll("&amp;", "&")
            .replaceAll("\\s+", " ").trim();
        int dot = text.indexOf(". ");
        String s = dot > 0 ? text.substring(0, dot + 1) : text;
        return s.length() > 160 ? s.substring(0, 157) + "..." : s;
    }

    private static Element child(Element e, String name) {
        NodeList list = e.getChildNodes();
        for (int i = 0; i < list.getLength(); i++) {
            Node n = list.item(i);
            if (n instanceof Element c && name.equals(c.getTagName())) {
                return c;
            }
        }
        return null;
    }

    private static String childText(Element e, String name) {
        Element c = child(e, name);
        return c == null ? null : c.getTextContent().trim();
    }
}
