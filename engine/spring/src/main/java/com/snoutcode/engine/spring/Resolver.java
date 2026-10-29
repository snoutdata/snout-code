package com.snoutcode.engine.spring;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.snoutcode.engine.spring.Facts.BeanDef;
import com.snoutcode.engine.spring.Facts.Endpoint;
import com.snoutcode.engine.spring.Facts.InjectionPoint;
import com.snoutcode.engine.spring.Facts.ScanSpec;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Deque;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Builds the application contexts of a workspace from its facts and resolves every injection
 * point against them. It evaluates no condition: a bean that carries one is a candidate that may
 * be absent, so it never makes an injection ambiguous. Profiles are evaluated when the project
 * says which are active ({@link ProjectInput#activeProfiles}): a bean whose {@code @Profile} does
 * not match is no candidate (it stays in the context's list, as inactive), and one that matches
 * counts as unconditional; otherwise a profile is a condition like the others.
 *
 * <p>A context is a project with everything on its classpath. Injection points resolve in the
 * contexts of the projects at the top of the dependency graph that include them, so code shared
 * by several applications is checked in each: a bean is missing only when no application has
 * one, and ambiguous when some application has two.
 */
final class Resolver {
    /** What the resolver asks of JDT. */
    interface Oracle {
        /** The facts of an XML bean file on a project's classpath ({@code classpath:app-config.xml}); null when absent. */
        Facts xml(String location, Collection<String> projects);

        /** An interface with annotations and no implementation: a library may register a proxy for it. */
        boolean proxyInterface(String project, String type);

        /** A library class annotated as a component: something may scan it. */
        boolean libraryComponent(String project, String type);
    }

    /** One JDT project's input. */
    static final class ProjectInput {
        String name;
        String module;
        final List<String> dependencies = new ArrayList<>();
        final List<Facts> facts = new ArrayList<>();
        final List<BeanDef> library = new ArrayList<>();
        /** The profiles active when this project starts the application; null when they are not evaluated. */
        Set<String> activeProfiles;
    }

    /** One application context: a project and its classpath. */
    static final class Context {
        String root;
        final Set<String> projects = new LinkedHashSet<>();
        /** Whether something starts it: a Spring Boot application, a component scan, XML. */
        boolean bootable;
        boolean scanAll;
        final Set<String> scanPackages = new LinkedHashSet<>();
        final Set<String> mapperPackages = new LinkedHashSet<>();
        final Set<String> classes = new LinkedHashSet<>();
        final List<BeanDef> beans = new ArrayList<>();
        final List<BeanDef> library = new ArrayList<>();
        final List<Endpoint> endpoints = new ArrayList<>();
        final List<ScanSpec> applications = new ArrayList<>();
        final Set<String> xmlFiles = new LinkedHashSet<>();
        final Map<String, List<BeanDef>> byType = new HashMap<>();
        final Map<String, List<BeanDef>> byName = new HashMap<>();
        /** Components in the projects that no scan reaches. */
        final List<BeanDef> unscanned = new ArrayList<>();
        /** The active profiles, when they are evaluated. */
        Set<String> activeProfiles;
        /** Beans whose {@code @Profile} the active profiles do not match: listed, never injected. */
        final Set<BeanDef> inactive = java.util.Collections.newSetFromMap(new IdentityHashMap<>());

        boolean active(BeanDef b) {
            return !inactive.contains(b);
        }

        /** Whether the bean may be absent: a condition, a library's, or a profile not evaluated. */
        boolean conditional(BeanDef b) {
            return !b.conditions.isEmpty() || "library".equals(b.kind) || (!b.profiles.isEmpty() && activeProfiles == null);
        }
    }

    /** What the resolver found. */
    static final class Result {
        final Map<String, Context> contexts = new LinkedHashMap<>();
        final List<InjectionPoint> injections = new ArrayList<>();
        /** Bean id to the injection points it is injected into. */
        final Map<String, List<InjectionPoint>> usages = new HashMap<>();
        /** Uri to diagnostics. */
        final Map<String, JsonArray> diagnostics = new LinkedHashMap<>();
    }

    private static final String DATA_REPOSITORY = "org.springframework.data.repository.Repository";
    private static final String NO_REPOSITORY_BEAN = "org.springframework.data.repository.NoRepositoryBean";
    static final String QUALIFIER = "org.springframework.beans.factory.annotation.Qualifier";

    /** Types the container itself supplies (its resolvable dependencies and infrastructure beans). */
    static final Set<String> IMPLICIT = Set.of(
        "org.springframework.beans.factory.BeanFactory",
        "org.springframework.beans.factory.ListableBeanFactory",
        "org.springframework.beans.factory.HierarchicalBeanFactory",
        "org.springframework.beans.factory.config.ConfigurableBeanFactory",
        "org.springframework.beans.factory.config.ConfigurableListableBeanFactory",
        "org.springframework.beans.factory.config.AutowireCapableBeanFactory",
        "org.springframework.beans.factory.InjectionPoint",
        "org.springframework.beans.factory.config.DependencyDescriptor",
        "org.springframework.context.ApplicationContext",
        "org.springframework.context.ConfigurableApplicationContext",
        "org.springframework.context.ApplicationEventPublisher",
        "org.springframework.context.MessageSource",
        "org.springframework.context.event.ApplicationEventMulticaster",
        "org.springframework.context.LifecycleProcessor",
        "org.springframework.core.io.ResourceLoader",
        "org.springframework.core.io.support.ResourcePatternResolver",
        "org.springframework.core.env.Environment",
        "org.springframework.core.env.ConfigurableEnvironment",
        "org.springframework.core.env.PropertyResolver",
        "org.springframework.boot.ApplicationArguments",
        "org.springframework.web.context.WebApplicationContext",
        "org.springframework.web.context.request.WebRequest",
        "org.springframework.web.context.request.NativeWebRequest",
        "org.springframework.boot.web.servlet.context.ServletWebServerApplicationContext",
        "jakarta.servlet.ServletContext", "jakarta.servlet.ServletRequest", "jakarta.servlet.ServletResponse",
        "jakarta.servlet.http.HttpServletRequest", "jakarta.servlet.http.HttpServletResponse", "jakarta.servlet.http.HttpSession",
        "javax.servlet.ServletContext", "javax.servlet.ServletRequest", "javax.servlet.ServletResponse",
        "javax.servlet.http.HttpServletRequest", "javax.servlet.http.HttpServletResponse", "javax.servlet.http.HttpSession");

    private final Map<String, ProjectInput> projects;
    private final Oracle oracle;

    Resolver(Map<String, ProjectInput> projects, Oracle oracle) {
        this.projects = projects;
        this.oracle = oracle;
    }

    Result resolve() {
        Result result = new Result();
        for (String name : projects.keySet()) {
            result.contexts.put(name, context(name));
        }
        Set<String> dependedOn = new HashSet<>();
        projects.values().forEach(p -> dependedOn.addAll(p.dependencies));
        List<Context> tops = new ArrayList<>();
        for (Context c : result.contexts.values()) {
            if (!dependedOn.contains(c.root)) {
                tops.add(c);
            }
        }
        for (ProjectInput p : projects.values()) {
            List<Context> relevant = new ArrayList<>();
            for (Context c : tops) {
                if (c.projects.contains(p.name)) {
                    relevant.add(c);
                }
            }
            for (Facts f : p.facts) {
                for (InjectionPoint i : f.injections) {
                    resolve(i, relevant);
                    result.injections.add(i);
                    for (BeanDef b : i.beans) {
                        result.usages.computeIfAbsent(b.id(), k -> new ArrayList<>()).add(i);
                    }
                    JsonObject d = diagnostic(i);
                    if (d != null && i.loc != null) {
                        result.diagnostics.computeIfAbsent(i.loc.uri, k -> new JsonArray()).add(d);
                    }
                }
            }
        }
        return result;
    }

    // ---- contexts

    private Set<String> closure(String root) {
        Set<String> out = new LinkedHashSet<>();
        Deque<String> work = new ArrayDeque<>();
        work.add(root);
        while (!work.isEmpty()) {
            String n = work.pop();
            ProjectInput p = projects.get(n);
            if (p != null && out.add(n)) {
                work.addAll(p.dependencies);
            }
        }
        return out;
    }

    Context context(String root) {
        Context c = new Context();
        c.root = root;
        c.projects.addAll(closure(root));
        List<Facts> all = new ArrayList<>();
        for (String n : c.projects) {
            all.addAll(projects.get(n).facts);
        }
        List<BeanDef> components = new ArrayList<>();
        List<BeanDef> methods = new ArrayList<>();
        List<BeanDef> interfaces = new ArrayList<>();
        List<ScanSpec> scans = new ArrayList<>();
        Map<String, Set<String>> imports = new HashMap<>();
        Map<String, Set<String>> resources = new HashMap<>();
        Map<String, String> enclosing = new HashMap<>();
        Set<String> configurations = new HashSet<>();
        Set<String> enabledProperties = new HashSet<>();
        for (Facts f : all) {
            for (BeanDef b : f.beans) {
                if (b.method != null) {
                    methods.add(b);
                } else {
                    components.add(b);
                    if (b.configuration) {
                        configurations.add(b.declaringType);
                    }
                }
            }
            interfaces.addAll(f.interfaces);
            scans.addAll(f.scans);
            f.imports.forEach((k, v) -> imports.computeIfAbsent(k, x -> new LinkedHashSet<>()).addAll(v));
            f.importResources.forEach((k, v) -> resources.computeIfAbsent(k, x -> new LinkedHashSet<>()).addAll(v));
            enclosing.putAll(f.enclosing);
            enabledProperties.addAll(f.enabledProperties);
        }

        // Where the application starts: its @SpringBootApplication classes, else every class with a scan.
        Set<String> seeds = new LinkedHashSet<>();
        for (ScanSpec s : scans) {
            if (s.application) {
                seeds.add(s.declaringType);
                c.applications.add(s);
            }
        }
        if (seeds.isEmpty()) {
            for (ScanSpec s : scans) {
                if ("componentScan".equals(s.kind)) {
                    seeds.add(s.declaringType);
                }
            }
        }
        c.bootable = !seeds.isEmpty();
        c.scanAll = seeds.isEmpty();
        c.classes.addAll(seeds);

        List<Facts> xmlFacts = new ArrayList<>();
        Set<BeanDef> in = java.util.Collections.newSetFromMap(new IdentityHashMap<>());
        boolean changed = true;
        while (changed) {
            changed = false;
            for (ScanSpec s : scans) {
                if (c.classes.contains(s.declaringType)) {
                    changed |= ("mapperScan".equals(s.kind) ? c.mapperPackages : c.scanPackages).addAll(s.packages);
                }
            }
            for (Facts x : xmlFacts) {
                for (ScanSpec s : x.scans) {
                    changed |= ("mapperScan".equals(s.kind) ? c.mapperPackages : c.scanPackages).addAll(s.packages);
                }
            }
            for (BeanDef b : components) {
                if (in.contains(b) || b.isAbstract) {
                    continue;
                }
                boolean member = c.classes.contains(b.declaringType)
                    || c.scanAll || inPackages(b.packageName, c.scanPackages)
                    || "properties".equals(b.kind)
                    || (enclosing.containsKey(b.declaringType) && b.configuration && c.classes.contains(enclosing.get(b.declaringType)));
                if (member) {
                    in.add(b);
                    c.classes.add(b.declaringType);
                    changed = true;
                }
            }
            for (String cls : new ArrayList<>(c.classes)) {
                Set<String> imported = imports.get(cls);
                if (imported != null) {
                    changed |= c.classes.addAll(imported);
                }
                for (String location : resources.getOrDefault(cls, Set.of())) {
                    changed |= loadXml(c, location, xmlFacts);
                }
            }
            for (int i = 0; i < xmlFacts.size(); i++) {
                for (Set<String> more : xmlFacts.get(i).importResources.values()) {
                    for (String location : more) {
                        changed |= loadXml(c, location, xmlFacts);
                    }
                }
            }
        }
        for (BeanDef b : components) {
            if (in.contains(b)) {
                c.beans.add(b);
            } else if (!b.isAbstract) {
                c.unscanned.add(b);
            }
        }
        for (BeanDef b : methods) {
            if (c.classes.contains(b.declaringType)) {
                c.beans.add(b);
            }
        }
        for (Facts x : xmlFacts) {
            c.beans.addAll(x.beans);
        }
        for (BeanDef b : interfaces) {
            boolean data = b.assignable.contains(DATA_REPOSITORY) && !b.conditions.contains(NO_REPOSITORY_BEAN)
                && !DATA_REPOSITORY.equals(b.type);
            boolean mapper = "mapper".equals(b.kind) || inPackages(b.packageName, c.mapperPackages);
            if (data || mapper) {
                BeanDef r = copy(b);
                r.kind = data ? "dataRepository" : "mapper";
                r.conditions.remove(NO_REPOSITORY_BEAN);
                c.beans.add(r);
            }
        }
        // A @ConfigurationProperties class named by @EnableConfigurationProperties is a bean too.
        for (BeanDef b : components) {
            if ("properties".equals(b.kind) && enabledProperties.contains(b.declaringType) && !c.beans.contains(b)) {
                c.beans.add(b);
            }
        }
        ProjectInput rootProject = projects.get(root);
        c.library.addAll(rootProject.library);
        c.activeProfiles = rootProject.activeProfiles;
        for (BeanDef b : c.beans) {
            if (c.activeProfiles != null && !Profiles.matches(b.profiles, c.activeProfiles)) {
                c.inactive.add(b);
                continue;
            }
            index(c, b);
        }
        for (BeanDef b : c.library) {
            index(c, b);
        }
        for (Facts f : all) {
            for (Endpoint e : f.endpoints) {
                if (c.classes.contains(e.controller)) {
                    c.endpoints.add(e);
                }
            }
        }
        return c;
    }

    private boolean loadXml(Context c, String location, List<Facts> xmlFacts) {
        if (!c.xmlFiles.add(location)) {
            return false;
        }
        Facts x = oracle.xml(location, c.projects);
        if (x == null) {
            return false;
        }
        xmlFacts.add(x);
        c.bootable = true;
        c.scanAll = false;
        for (BeanDef b : x.beans) {
            if (b.declaringType != null) {
                c.classes.add(b.declaringType);
            }
        }
        return true;
    }

    private static BeanDef copy(BeanDef b) {
        BeanDef r = new BeanDef();
        r.name = b.name;
        r.aliases.addAll(b.aliases);
        r.type = b.type;
        r.assignable.addAll(b.assignable);
        r.parameterized.putAll(b.parameterized);
        r.kind = b.kind;
        r.stereotype = b.stereotype;
        r.declaringType = b.declaringType;
        r.packageName = b.packageName;
        r.loc = b.loc;
        r.language = b.language;
        r.project = b.project;
        r.primary = b.primary;
        r.isInterface = true;
        r.qualifiers.addAll(b.qualifiers);
        r.profiles.addAll(b.profiles);
        r.conditions.addAll(b.conditions);
        return r;
    }

    private static void index(Context c, BeanDef b) {
        for (String t : b.assignable) {
            c.byType.computeIfAbsent(t, k -> new ArrayList<>()).add(b);
        }
        c.byName.computeIfAbsent(b.name, k -> new ArrayList<>()).add(b);
        for (String a : b.aliases) {
            c.byName.computeIfAbsent(a, k -> new ArrayList<>()).add(b);
        }
    }

    static boolean inPackages(String pkg, Collection<String> packages) {
        if (pkg == null) {
            return false;
        }
        for (String p : packages) {
            if (p.isEmpty() || pkg.equals(p) || pkg.startsWith(p + ".")) {
                return true;
            }
        }
        return false;
    }

    // ---- resolution

    private static final class Outcome {
        String status;
        final List<BeanDef> beans = new ArrayList<>();
        String note;
    }

    private void resolve(InjectionPoint i, List<Context> contexts) {
        i.beans.clear();
        if ("value".equals(i.annotation)) {
            i.status = "value";
            return;
        }
        if (i.unresolved || i.type == null) {
            i.status = "unresolved";
            return;
        }
        if (IMPLICIT.contains(i.type)) {
            i.status = "framework";
            return;
        }
        if (contexts.isEmpty()) {
            i.status = "unknown";
            return;
        }
        List<Context> preferred = new ArrayList<>();
        for (Context c : contexts) {
            if (c.classes.contains(i.declaringType) || c.beans.stream().anyMatch(b -> b.method == null && b.assignable.contains(i.declaringType))) {
                preferred.add(c);
            }
        }
        if (preferred.isEmpty()) {
            preferred = contexts;
        }
        List<Outcome> outcomes = new ArrayList<>();
        for (Context c : preferred) {
            outcomes.add(resolveIn(i, c));
        }
        Set<BeanDef> beans = new LinkedHashSet<>();
        outcomes.forEach(o -> beans.addAll(o.beans));
        i.beans.addAll(beans);
        Outcome ambiguous = outcomes.stream().filter(o -> "ambiguous".equals(o.status)).findFirst().orElse(null);
        boolean allMissing = outcomes.stream().allMatch(o -> "missing".equals(o.status));
        if (ambiguous != null) {
            i.status = "ambiguous";
            i.beans.clear();
            i.beans.addAll(ambiguous.beans);
            i.message = ambiguousMessage(i);
        } else if (allMissing) {
            i.status = "missing";
            i.message = "No bean of type '" + i.typeDisplay() + "'" + qualifierText(i) + " found"
                + outcomes.stream().map(o -> o.note).filter(n -> n != null).findFirst().map(n -> ": " + n).orElse("");
        } else {
            String best = "unknown";
            for (String s : List.of("resolved", "multiple", "possible", "optional", "unknown")) {
                if (outcomes.stream().anyMatch(o -> s.equals(o.status))) {
                    best = s;
                    break;
                }
            }
            i.status = best;
        }
    }

    private Outcome resolveIn(InjectionPoint i, Context c) {
        Outcome o = new Outcome();
        if ("resource".equals(i.annotation)) {
            String name = i.resourceName != null ? i.resourceName : i.name;
            List<BeanDef> named = name == null ? List.of() : c.byName.getOrDefault(name, List.of());
            if (!named.isEmpty()) {
                o.status = "resolved";
                o.beans.addAll(named);
                return o;
            }
        }
        List<BeanDef> candidates = new ArrayList<>();
        for (BeanDef b : c.byType.getOrDefault(i.type, List.of())) {
            if (b.autowireCandidate && genericMatch(i, b) && qualifies(i, b)) {
                candidates.add(b);
            }
        }
        // Spring does not inject a bean into itself while others match.
        if (candidates.size() > 1) {
            candidates.removeIf(b -> b.method == null && i.declaringType.equals(b.declaringType));
        }
        if ("collection".equals(i.wrapper) || "map".equals(i.wrapper) || "array".equals(i.wrapper)) {
            o.status = "multiple";
            o.beans.addAll(candidates);
            return o;
        }
        if (candidates.isEmpty()) {
            // A @Bean method may declare a supertype of what it returns.
            for (String t : i.typeSupers) {
                for (BeanDef b : c.byType.getOrDefault(t, List.of())) {
                    if (b.type.equals(t) && b.method != null && qualifies(i, b)) {
                        o.beans.add(b);
                    }
                }
            }
            if (!o.beans.isEmpty()) {
                o.status = "possible";
                return o;
            }
            if (!i.required || "optional".equals(i.wrapper) || "provider".equals(i.wrapper)) {
                o.status = "optional";
                return o;
            }
            if (!c.bootable || oracle.proxyInterface(c.root, i.type) || oracle.libraryComponent(c.root, i.type)) {
                o.status = "unknown";
                return o;
            }
            o.status = "missing";
            for (BeanDef b : c.unscanned) {
                if (b.assignable.contains(i.type)) {
                    o.note = Names.display(b.declaringType) + " is not in a scanned package";
                    break;
                }
            }
            return o;
        }
        if (candidates.size() == 1) {
            o.status = "resolved";
            o.beans.add(candidates.get(0));
            return o;
        }
        List<BeanDef> narrowed = narrow(i, candidates, c);
        o.beans.addAll(narrowed.isEmpty() ? candidates : narrowed);
        o.status = narrowed.size() == 1 || "provider".equals(i.wrapper) ? "resolved" : "ambiguous";
        if (narrowed.size() > 1 && narrowed.stream().filter(b -> !c.conditional(b)).count() < 2) {
            o.status = "resolved";
        }
        return o;
    }

    /** Spring's choice among several candidates: the primary one, else not the fallbacks, else the one named like the field. */
    private static List<BeanDef> narrow(InjectionPoint i, List<BeanDef> candidates, Context c) {
        List<BeanDef> primary = candidates.stream().filter(b -> b.primary).toList();
        if (primary.size() == 1) {
            return primary;
        }
        List<BeanDef> rest = candidates.stream().filter(b -> !b.fallback).toList();
        if (rest.size() == 1) {
            return rest;
        }
        if (i.name != null) {
            List<BeanDef> named = candidates.stream().filter(b -> i.name.equals(b.name) || b.aliases.contains(i.name)).toList();
            if (named.size() == 1) {
                return named;
            }
        }
        List<BeanDef> unconditional = candidates.stream().filter(b -> !c.conditional(b)).toList();
        if (unconditional.size() == 1) {
            return unconditional;
        }
        return candidates;
    }

    private static boolean qualifies(InjectionPoint i, BeanDef b) {
        for (String q : i.qualifiers) {
            if (!(b.qualifiers.contains(q) || q.equals(b.name) || b.aliases.contains(q))) {
                return false;
            }
        }
        return true;
    }

    /** {@code Repository<User>} does not take a bean that is a {@code Repository<Order>}. */
    private static boolean genericMatch(InjectionPoint i, BeanDef b) {
        if (i.fullType == null || i.fullType.indexOf('<') < 0) {
            return true;
        }
        String declared = b.parameterized.get(i.type);
        if (declared == null || i.fullType.contains("?")) {
            return true;
        }
        return declared.equals(i.fullType);
    }

    // ---- diagnostics

    private static String qualifierText(InjectionPoint i) {
        if (i.qualifiers.isEmpty()) {
            return "";
        }
        return " qualified '" + String.join("', '", i.qualifiers) + "'";
    }

    private static String ambiguousMessage(InjectionPoint i) {
        List<String> names = new ArrayList<>();
        for (BeanDef b : i.beans) {
            names.add(b.name);
        }
        return names.size() + " beans of type '" + i.typeDisplay() + "'" + qualifierText(i) + " match: "
            + String.join(", ", names) + ". Mark one @Primary, or choose one with @Qualifier";
    }

    static JsonObject diagnostic(InjectionPoint i) {
        if (!"missing".equals(i.status) && !"ambiguous".equals(i.status)) {
            return null;
        }
        if (i.loc == null) {
            return null;
        }
        JsonObject d = new JsonObject();
        d.add("range", i.loc.range());
        d.addProperty("severity", 2);
        d.addProperty("source", "spring");
        d.addProperty("code", "missing".equals(i.status) ? "spring.missingBean" : "spring.ambiguousBean");
        d.addProperty("message", i.message);
        JsonArray fixes = fixes(i);
        if (!fixes.isEmpty()) {
            JsonObject data = new JsonObject();
            data.add("fixes", fixes);
            d.add("data", data);
        }
        return d;
    }

    /** Quick fixes, each {@code {title, edits: [TextEdit]}} in the injection point's file. */
    static JsonArray fixes(InjectionPoint i) {
        JsonArray out = new JsonArray();
        JsonObject fix = i.fix;
        if (fix == null) {
            return out;
        }
        if ("ambiguous".equals(i.status) && fix.has("qualifierLine")) {
            Set<String> seen = new HashSet<>();
            for (BeanDef b : i.beans) {
                if (!seen.add(b.name) || seen.size() > 6) {
                    continue;
                }
                JsonArray edits = new JsonArray();
                int line = fix.get("qualifierLine").getAsInt();
                int character = fix.get("qualifierCharacter").getAsInt();
                String text = "@Qualifier(\"" + b.name + "\")"
                    + (fix.get("qualifierInline").getAsBoolean() ? " " : "\n" + fix.get("indent").getAsString());
                edits.add(edit(line, character, line, character, text));
                if (fix.has("importLine")) {
                    int il = fix.get("importLine").getAsInt();
                    edits.add(edit(il, 0, il, 0, "import " + QUALIFIER + ";\n"));
                }
                JsonObject f = new JsonObject();
                f.addProperty("title", "Inject '" + b.name + "' with @Qualifier");
                f.add("edits", edits);
                out.add(f);
            }
        }
        if ("missing".equals(i.status) && fix.has("autowired")) {
            JsonObject range = fix.getAsJsonObject("autowired");
            JsonArray edits = new JsonArray();
            JsonObject e = new JsonObject();
            e.add("range", range);
            e.addProperty("newText", "@Autowired(required = false)");
            edits.add(e);
            JsonObject f = new JsonObject();
            f.addProperty("title", "Make it optional: @Autowired(required = false)");
            f.add("edits", edits);
            out.add(f);
        }
        return out;
    }

    private static JsonObject edit(int line, int character, int endLine, int endCharacter, String text) {
        JsonObject e = new JsonObject();
        e.add("range", Loc.range(line, character, endLine, endCharacter));
        e.addProperty("newText", text);
        return e;
    }
}
