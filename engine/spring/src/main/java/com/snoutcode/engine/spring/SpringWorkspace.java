package com.snoutcode.engine.spring;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.snoutcode.engine.jdtls.ImportPlan;
import com.snoutcode.engine.jdtls.SnoutProjectImporter;
import com.snoutcode.engine.spring.Facts.BeanDef;
import com.snoutcode.engine.spring.Facts.Endpoint;
import com.snoutcode.engine.spring.Facts.InjectionPoint;
import com.snoutcode.engine.spring.Facts.KeyUse;
import com.snoutcode.engine.spring.Facts.ScanSpec;
import org.eclipse.core.resources.ResourcesPlugin;
import org.eclipse.core.runtime.IPath;
import org.eclipse.core.runtime.IProgressMonitor;
import org.eclipse.jdt.core.IAnnotation;
import org.eclipse.jdt.core.IClasspathEntry;
import org.eclipse.jdt.core.ICompilationUnit;
import org.eclipse.jdt.core.IField;
import org.eclipse.jdt.core.IJavaElement;
import org.eclipse.jdt.core.IJavaProject;
import org.eclipse.jdt.core.IMethod;
import org.eclipse.jdt.core.IOrdinaryClassFile;
import org.eclipse.jdt.core.IPackageFragment;
import org.eclipse.jdt.core.IPackageFragmentRoot;
import org.eclipse.jdt.core.IType;
import org.eclipse.jdt.core.JavaCore;
import org.eclipse.jdt.core.JavaModelException;
import org.eclipse.jdt.core.dom.AST;
import org.eclipse.jdt.core.dom.ASTParser;
import org.eclipse.jdt.core.dom.ASTRequestor;
import org.eclipse.jdt.core.dom.AbstractTypeDeclaration;
import org.eclipse.jdt.core.dom.CompilationUnit;
import org.eclipse.jdt.core.dom.IAnnotationBinding;
import org.eclipse.jdt.core.dom.IBinding;
import org.eclipse.jdt.core.dom.IMethodBinding;
import org.eclipse.jdt.core.dom.ITypeBinding;
import org.eclipse.jdt.core.dom.IVariableBinding;
import org.eclipse.jdt.ls.core.internal.JDTUtils;
import org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin;
import org.eclipse.jdt.ls.core.internal.ResourceUtils;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * The Spring model of the jdtls workspace: the facts of every Java compilation unit that may
 * hold Spring code, of the Scala bridge's classes and of XML bean files, the beans the
 * libraries contribute, and the resolution of all of it. snout-engine drives it through
 * {@link SpringCommands}: one scan when jdtls is ready, then updates of the files that change.
 */
final class SpringWorkspace {
    static final SpringWorkspace INSTANCE = new SpringWorkspace();

    private static final Pattern RELEVANT = Pattern.compile("@(?:Component|Service|Repository|Controller|RestController|Configuration"
        + "|Bean|Autowired|Inject|Value|Resource|RequestMapping|[A-Z]\\w*Mapping|ConfigurationProperties\\w*|SpringBootApplication"
        + "|EnableAutoConfiguration|ComponentScans?|Import|ImportResource|Mapper|MapperScan|Named|Primary|Qualifier|ConditionalOn\\w*"
        + "|Enable\\w+|AutoConfiguration|\\w*(?:Controller|Service|Component|Configuration|Repository|Application|Advice|Endpoint))\\b"
        + "|\\bextends\\s+[\\w.<>, ]*Repository\\b|@interface\\b");
    private static final Pattern GET_PROPERTY = Pattern.compile("\\b(?:getProperty|getRequiredProperty|containsProperty)\\(\\s*\"([^\"]+)\"");
    private static final Set<String> SCALA_ANNOTATIONS = Set.of("scala.reflect.ScalaSignature", "scala.reflect.ScalaLongSignature",
        "scala.reflect.ScalaSignature$", "ScalaSignature", "ScalaLongSignature");
    private static final Set<String> STEREOTYPES = Set.of(Annotations.COMPONENT, Annotations.SERVICE, Annotations.REPOSITORY,
        Annotations.CONTROLLER, Annotations.REST_CONTROLLER, Annotations.CONFIGURATION,
        "org.springframework.boot.autoconfigure.AutoConfiguration", "org.springframework.web.bind.annotation.ControllerAdvice",
        "org.springframework.web.bind.annotation.RestControllerAdvice");

    static final class ProjectState {
        String name;
        IJavaProject java;
        ImportPlan.Project spec;
        boolean spring;
        final List<String> dependencies = new ArrayList<>();
        /** Compilation unit handle to its facts. */
        final Map<String, Facts> units = new LinkedHashMap<>();
        final Map<String, String> unitUris = new HashMap<>();
        Facts binary = new Facts();
        final List<IPackageFragmentRoot> bridgeRoots = new ArrayList<>();
        List<BeanDef> library;
    }

    private final Locations locations = new Locations();
    private final Map<String, ProjectState> projects = new LinkedHashMap<>();
    private final Map<String, Object[]> xmlCache = new HashMap<>();
    private final Map<String, List<BeanDef>> libraryCache = new HashMap<>();
    private final Map<String, Object[]> jarMetadata = new HashMap<>();
    private final Map<String, List<String>> enumCache = new HashMap<>();
    private Resolver.Result result;
    private Map<String, JsonArray> published = new HashMap<>();
    private boolean scanned;
    private final JsonObject stats = new JsonObject();
    /**
     * How profiles are evaluated: {@code any} (not at all: a bean with a profile may be absent),
     * {@code config} (the application files' {@code spring.profiles.active}), or {@code chosen}
     * ({@link #chosenProfiles}, as a command line would).
     */
    private String profileMode = "any";
    private List<String> chosenProfiles = List.of();

    private SpringWorkspace() {
    }

    // ---- scanning

    /** Scans everything again and answers every file's diagnostics. */
    synchronized JsonObject scan(IProgressMonitor monitor) throws Exception {
        long start = System.currentTimeMillis();
        projects.clear();
        libraryCache.clear();
        enumCache.clear();
        locations.clearScala();
        Map<String, ImportPlan.Project> specs = specs();
        for (IJavaProject java : JavaCore.create(ResourcesPlugin.getWorkspace().getRoot()).getJavaProjects()) {
            if (!java.getProject().isOpen()) {
                continue;
            }
            ProjectState p = new ProjectState();
            p.name = java.getElementName();
            p.java = java;
            p.spec = specs.get(p.name);
            p.spring = java.findType(Annotations.COMPONENT) != null || java.findType(Annotations.BEAN) != null;
            for (IClasspathEntry e : java.getRawClasspath()) {
                if (e.getEntryKind() == IClasspathEntry.CPE_PROJECT) {
                    p.dependencies.add(e.getPath().lastSegment());
                }
            }
            projects.put(p.name, p);
        }
        int units = 0;
        for (ProjectState p : projects.values()) {
            if (p.spring) {
                units += scanSources(p, monitor);
                scanBinaries(p, monitor);
            }
        }
        units += scanMapperPackages(monitor);
        long scanMillis = System.currentTimeMillis() - start;
        resolve();
        scanned = true;
        published = new HashMap<>(result.diagnostics);
        stats.addProperty("units", units);
        stats.addProperty("scanMillis", scanMillis);
        stats.addProperty("millis", System.currentTimeMillis() - start);
        JsonObject out = new JsonObject();
        out.add("diagnostics", toJson(published));
        out.add("stats", summary());
        JavaLanguageServerPlugin.logInfo("SnoutCode Spring: scanned " + units + " compilation unit(s) in " + scanMillis + " ms, "
            + summary());
        return out;
    }

    private static Map<String, ImportPlan.Project> specs() {
        Map<String, ImportPlan.Project> out = new HashMap<>();
        String model = System.getProperty(SnoutProjectImporter.MODEL_PROPERTY);
        if (model == null || !Files.isRegularFile(Path.of(model))) {
            return out;
        }
        try {
            for (ImportPlan.Project p : ImportPlan.read(Path.of(model)).projects) {
                out.put(p.name, p);
            }
        } catch (IOException | RuntimeException e) {
            JavaLanguageServerPlugin.logException("SnoutCode Spring: cannot read the model", e);
        }
        return out;
    }

    void ensureScanned(IProgressMonitor monitor) throws Exception {
        if (!scanned) {
            scan(monitor);
        }
    }

    private int scanSources(ProjectState p, IProgressMonitor monitor) throws JavaModelException {
        p.units.clear();
        p.unitUris.clear();
        List<ICompilationUnit> all = new ArrayList<>();
        for (IPackageFragmentRoot root : p.java.getPackageFragmentRoots()) {
            if (root.getKind() != IPackageFragmentRoot.K_SOURCE || isTest(root)) {
                continue;
            }
            for (IJavaElement child : root.getChildren()) {
                if (child instanceof IPackageFragment pkg) {
                    all.addAll(List.of(pkg.getCompilationUnits()));
                }
            }
        }
        // Custom stereotypes: annotation types declared here that carry a Spring annotation.
        Set<String> custom = new HashSet<>();
        Map<ICompilationUnit, String> texts = new HashMap<>();
        for (ICompilationUnit cu : all) {
            String text = source(cu);
            texts.put(cu, text);
            if (text.contains("@interface") && RELEVANT.matcher(text.replace("@interface", "")).find()) {
                Matcher m = Pattern.compile("@interface\\s+(\\w+)").matcher(text);
                while (m.find()) {
                    custom.add(m.group(1));
                }
            }
        }
        Pattern customPattern = custom.isEmpty() ? null : Pattern.compile("@(?:" + String.join("|", custom) + ")\\b");
        List<ICompilationUnit> candidates = new ArrayList<>();
        for (ICompilationUnit cu : all) {
            String text = texts.get(cu);
            if (RELEVANT.matcher(text).find() || (customPattern != null && customPattern.matcher(text).find())) {
                candidates.add(cu);
            } else {
                Facts keys = new Facts();
                propertyCalls(text, JDTUtils.toURI(cu), keys);
                if (!keys.isEmpty()) {
                    p.units.put(cu.getHandleIdentifier(), keys);
                    p.unitUris.put(cu.getHandleIdentifier(), JDTUtils.toURI(cu));
                }
            }
        }
        parse(p, candidates, texts, monitor);
        return candidates.size();
    }

    private static boolean isTest(IPackageFragmentRoot root) {
        try {
            IClasspathEntry entry = root.getRawClasspathEntry();
            return entry != null && entry.isTest();
        } catch (JavaModelException e) {
            return false;
        }
    }

    private static String source(ICompilationUnit cu) {
        try {
            String s = cu.getSource();
            return s == null ? "" : s;
        } catch (JavaModelException e) {
            return "";
        }
    }

    private void parse(ProjectState p, List<ICompilationUnit> units, Map<ICompilationUnit, String> texts, IProgressMonitor monitor) {
        if (units.isEmpty()) {
            return;
        }
        ASTParser parser = ASTParser.newParser(AST.getJLSLatest());
        parser.setProject(p.java);
        parser.setResolveBindings(true);
        parser.setBindingsRecovery(true);
        parser.setIgnoreMethodBodies(true);
        parser.setKind(ASTParser.K_COMPILATION_UNIT);
        // Batches keep the binding environment, and the memory, of each one bounded.
        for (int from = 0; from < units.size(); from += 300) {
            List<ICompilationUnit> batch = units.subList(from, Math.min(units.size(), from + 300));
            parser.setProject(p.java);
            parser.setResolveBindings(true);
            parser.setBindingsRecovery(true);
            parser.setIgnoreMethodBodies(true);
            parser.createASTs(batch.toArray(new ICompilationUnit[0]), new String[0], new ASTRequestor() {
                @Override
                public void acceptAST(ICompilationUnit source, CompilationUnit ast) {
                    String text = texts != null && texts.containsKey(source) ? texts.get(source) : SpringWorkspace.source(source);
                    try {
                        Facts facts = scanUnit(p, source, ast, text);
                        p.units.put(source.getHandleIdentifier(), facts);
                        p.unitUris.put(source.getHandleIdentifier(), JDTUtils.toURI(source));
                    } catch (RuntimeException e) {
                        JavaLanguageServerPlugin.logException("SnoutCode Spring: cannot read " + source.getElementName(), e);
                    }
                }
            }, monitor);
        }
    }

    private Facts scanUnit(ProjectState p, ICompilationUnit cu, CompilationUnit ast, String text) {
        Facts facts = new Facts();
        String uri = JDTUtils.toURI(cu);
        SourcePositions positions = new SourcePositions(ast, uri, text, locations);
        TypeScanner scanner = new TypeScanner(facts, positions, p.name, "java");
        for (Object t : ast.types()) {
            ITypeBinding b = ((AbstractTypeDeclaration) t).resolveBinding();
            if (b != null) {
                scanner.scan(b);
            }
        }
        propertyCalls(text, uri, facts);
        return facts;
    }

    /** Keys read with {@code environment.getProperty("key")}: known keys for the properties files. */
    private static void propertyCalls(String text, String uri, Facts facts) {
        Matcher m = GET_PROPERTY.matcher(text);
        LineIndex lines = null;
        while (m.find()) {
            if (lines == null) {
                lines = new LineIndex(text);
            }
            KeyUse k = new KeyUse();
            k.key = m.group(1);
            k.loc = Loc.of(uri, lines, m.start(1), m.group(1).length());
            facts.keyUses.add(k);
        }
    }

    /** MyBatis mapper packages hold plain interfaces the text filter does not pick: read them too. */
    private int scanMapperPackages(IProgressMonitor monitor) throws JavaModelException {
        Set<String> packages = new LinkedHashSet<>();
        for (ProjectState p : projects.values()) {
            for (Facts f : facts(p)) {
                for (ScanSpec s : f.scans) {
                    if ("mapperScan".equals(s.kind)) {
                        packages.addAll(s.packages);
                    }
                }
            }
        }
        if (packages.isEmpty()) {
            return 0;
        }
        int count = 0;
        for (ProjectState p : projects.values()) {
            if (!p.spring) {
                continue;
            }
            List<ICompilationUnit> more = new ArrayList<>();
            for (IPackageFragmentRoot root : p.java.getPackageFragmentRoots()) {
                if (root.getKind() != IPackageFragmentRoot.K_SOURCE || isTest(root)) {
                    continue;
                }
                for (IJavaElement child : root.getChildren()) {
                    if (child instanceof IPackageFragment pkg && Resolver.inPackages(pkg.getElementName(), packages)) {
                        for (ICompilationUnit cu : pkg.getCompilationUnits()) {
                            Facts existing = p.units.get(cu.getHandleIdentifier());
                            if (existing == null || (existing.beans.isEmpty() && existing.interfaces.isEmpty() && existing.injections.isEmpty())) {
                                more.add(cu);
                            }
                        }
                    }
                }
            }
            parse(p, more, null, monitor);
            count += more.size();
        }
        return count;
    }

    // ---- the Scala bridge's classes

    private void scanBinaries(ProjectState p, IProgressMonitor monitor) throws JavaModelException {
        p.binary = new Facts();
        p.bridgeRoots.clear();
        List<IType> types = new ArrayList<>();
        for (IPackageFragmentRoot root : p.java.getPackageFragmentRoots()) {
            if (root.getKind() != IPackageFragmentRoot.K_BINARY || root.isArchive() || isTest(root)) {
                continue;
            }
            IClasspathEntry entry = root.getRawClasspathEntry();
            if (entry == null || entry.getEntryKind() != IClasspathEntry.CPE_LIBRARY) {
                continue;
            }
            p.bridgeRoots.add(root);
            IPath sources = entry.getSourceAttachmentPath();
            if (sources != null) {
                locations.bridge(root.getPath().toOSString(), sources.toFile().toPath());
                scalaPropertyCalls(sources.toFile().toPath(), p.binary);
            }
            for (IJavaElement child : root.getChildren()) {
                if (!(child instanceof IPackageFragment pkg)) {
                    continue;
                }
                for (IOrdinaryClassFile cf : pkg.getOrdinaryClassFiles()) {
                    IType type = cf.getType();
                    if (type.getElementName().contains("$") || cf.getElementName().contains("$")) {
                        continue;
                    }
                    if (annotated(type)) {
                        types.add(type);
                    }
                }
            }
        }
        if (types.isEmpty()) {
            return;
        }
        BinaryPositions positions = new BinaryPositions(locations);
        TypeScanner scanner = new TypeScanner(p.binary, positions, p.name, "scala");
        for (int from = 0; from < types.size(); from += 400) {
            List<IType> batch = types.subList(from, Math.min(types.size(), from + 400));
            for (IBinding b : bindings(p.java, batch.toArray(new IJavaElement[0]), monitor)) {
                if (b instanceof ITypeBinding t) {
                    scanner.scan(t);
                }
            }
        }
    }

    /** {@code getProperty("key")} in the Scala sources, as for Java. */
    private static void scalaPropertyCalls(Path sources, Facts facts) {
        if (!Files.isDirectory(sources)) {
            return;
        }
        try (java.util.stream.Stream<Path> files = Files.walk(sources)) {
            files.filter(f -> f.toString().endsWith(".scala")).forEach(f -> {
                try {
                    String text = Files.readString(f, StandardCharsets.UTF_8);
                    if (text.contains("Property(")) {
                        propertyCalls(text, ResourceUtils.fixURI(f.toUri()), facts);
                    }
                } catch (IOException | RuntimeException e) {
                    // unreadable: skip
                }
            });
        } catch (IOException e) {
            // skip the folder
        }
    }

    private static boolean annotated(IType type) throws JavaModelException {
        for (IAnnotation a : type.getAnnotations()) {
            if (!SCALA_ANNOTATIONS.contains(a.getElementName())) {
                return true;
            }
        }
        for (IField f : type.getFields()) {
            if (f.getAnnotations().length > 0) {
                return true;
            }
        }
        for (IMethod m : type.getMethods()) {
            if (m.getAnnotations().length > 0) {
                return true;
            }
        }
        for (IType member : type.getTypes()) {
            if (annotated(member)) {
                return true;
            }
        }
        return false;
    }

    static IBinding[] bindings(IJavaProject project, IJavaElement[] elements, IProgressMonitor monitor) {
        if (elements.length == 0) {
            return new IBinding[0];
        }
        ASTParser parser = ASTParser.newParser(AST.getJLSLatest());
        parser.setProject(project);
        parser.setResolveBindings(true);
        parser.setBindingsRecovery(true);
        return parser.createBindings(elements, monitor);
    }

    /** Positions of the Scala bridge's classes, in their Scala files. */
    private static final class BinaryPositions implements TypeScanner.Positions {
        private final Locations locations;

        BinaryPositions(Locations locations) {
            this.locations = locations;
        }

        @Override
        public Loc type(ITypeBinding t) {
            return locations.of(t);
        }

        @Override
        public Loc field(IVariableBinding f) {
            return locations.of(f);
        }

        @Override
        public Loc method(IMethodBinding m) {
            return locations.of(m);
        }

        @Override
        public Loc param(IMethodBinding m, int index) {
            return locations.parameter(m, index);
        }
    }

    // ---- updates

    /** Reads the given files again ({@code .java}, {@code .scala}, XML) and answers the diagnostics that changed. */
    synchronized JsonObject update(List<String> uris, IProgressMonitor monitor) throws Exception {
        if (!scanned) {
            return scan(monitor);
        }
        long start = System.currentTimeMillis();
        boolean binaries = false;
        boolean mappers = false;
        for (String uri : uris) {
            String path = path(uri);
            if (path.endsWith(".java")) {
                mappers |= updateUnit(uri, monitor);
            } else if (path.endsWith(".scala") || path.endsWith(".class")) {
                binaries = true;
                locations.forget(ResourceUtils.fixURI(new File(path).toURI()));
            }
        }
        if (binaries) {
            locations.clearScala();
            for (ProjectState p : projects.values()) {
                if (p.spring) {
                    scanBinaries(p, monitor);
                }
            }
        }
        if (mappers) {
            scanMapperPackages(monitor);
        }
        resolve();
        JsonObject out = changedDiagnostics();
        out.addProperty("millis", System.currentTimeMillis() - start);
        return out;
    }

    /** The diagnostics that changed since they were last answered, as {@code {diagnostics}}. */
    private JsonObject changedDiagnostics() {
        Map<String, JsonArray> changed = new LinkedHashMap<>();
        Set<String> all = new LinkedHashSet<>(published.keySet());
        all.addAll(result.diagnostics.keySet());
        for (String uri : all) {
            JsonArray now = result.diagnostics.getOrDefault(uri, new JsonArray());
            if (!now.equals(published.getOrDefault(uri, new JsonArray()))) {
                changed.put(uri, now);
            }
        }
        published = new HashMap<>(result.diagnostics);
        JsonObject out = new JsonObject();
        out.add("diagnostics", toJson(changed));
        return out;
    }

    // ---- profiles

    /** The profile settings of a module's application files. */
    private static Profiles.Config profileConfig(ProjectState p) {
        return Profiles.read(resourceDirs(p, false));
    }

    /** The profiles active in a module, or null while they are not evaluated. */
    private Set<String> activeProfiles(ProjectState p) {
        return switch (profileMode) {
            case "config" -> Profiles.active(profileConfig(p), null);
            case "chosen" -> Profiles.active(profileConfig(p), chosenProfiles);
            default -> null;
        };
    }

    /**
     * {@code {mode: "any" | "config" | "chosen", profiles?}}: how profiles are evaluated from now
     * on. Resolves again; answers the diagnostics that changed and what {@link #profiles} answers.
     */
    synchronized JsonObject setProfiles(String mode, List<String> profiles, IProgressMonitor monitor) throws Exception {
        ensureScanned(monitor);
        profileMode = mode == null || !List.of("any", "config", "chosen").contains(mode) ? "any" : mode;
        chosenProfiles = profiles == null ? List.of() : List.copyOf(profiles);
        resolve();
        JsonObject out = changedDiagnostics();
        out.add("profiles", profiles(null, monitor));
        return out;
    }

    /** An application file changed: evaluate the profiles again, as they are set. */
    synchronized JsonObject refreshProfiles(IProgressMonitor monitor) throws Exception {
        if ("any".equals(profileMode)) {
            JsonObject out = new JsonObject();
            out.add("diagnostics", new JsonObject());
            return out;
        }
        return setProfiles(profileMode, chosenProfiles, monitor);
    }

    /**
     * The profiles each Spring module knows (from its application files' names and documents,
     * their settings and groups, and the {@code @Profile}s of its beans) and the ones active
     * now: {@code {mode, chosen, modules: [{module, configActive, active?, known: [{name, sources}]}]}}.
     */
    synchronized JsonObject profiles(String module, IProgressMonitor monitor) throws Exception {
        ensureScanned(monitor);
        JsonArray modules = new JsonArray();
        for (ProjectState p : modules(module)) {
            Profiles.Config config = profileConfig(p);
            Resolver.Context c = result.contexts.get(p.name);
            for (BeanDef b : beansOf(p)) {
                for (String expr : b.profiles) {
                    for (String n : Profiles.names(expr)) {
                        config.know(n, "@Profile on " + Names.display(b.method != null ? b.declaringType : b.type));
                    }
                }
            }
            JsonObject m = new JsonObject();
            m.addProperty("module", moduleOf(p));
            m.add("configActive", Facts.strings(config.list(Profiles.ACTIVE)));
            Set<String> active = activeProfiles(p);
            if (active != null) {
                m.add("active", Facts.strings(new ArrayList<>(active)));
            }
            JsonArray known = new JsonArray();
            config.known.forEach((name, sources) -> {
                JsonObject k = new JsonObject();
                k.addProperty("name", name);
                k.add("sources", Facts.strings(new ArrayList<>(sources)));
                known.add(k);
            });
            m.add("known", known);
            if (c != null) {
                m.addProperty("inactiveBeans", c.inactive.size());
            }
            modules.add(m);
        }
        JsonObject out = new JsonObject();
        out.addProperty("mode", profileMode);
        out.add("chosen", Facts.strings(chosenProfiles));
        out.add("modules", modules);
        return out;
    }

    /** The beans of a module's context. */
    private List<BeanDef> beansOf(ProjectState p) {
        Resolver.Context c = result == null ? null : result.contexts.get(p.name);
        return c == null ? List.of() : c.beans;
    }

    /** Whether a bean is out of every context it is listed in for its profiles. */
    private boolean inactiveEverywhere(BeanDef b) {
        boolean listed = false;
        for (Resolver.Context c : result.contexts.values()) {
            if (c.beans.contains(b)) {
                listed = true;
                if (c.active(b)) {
                    return false;
                }
            }
        }
        return listed;
    }

    /** Reads one compilation unit again; true when its mapper scans changed. */
    private boolean updateUnit(String uri, IProgressMonitor monitor) {
        String path = path(uri);
        ICompilationUnit cu = JDTUtils.resolveCompilationUnit(uri);
        boolean hadMapperScan = false;
        for (ProjectState p : projects.values()) {
            for (Map.Entry<String, String> e : new ArrayList<>(p.unitUris.entrySet())) {
                if (path(e.getValue()).equals(path)) {
                    Facts old = p.units.remove(e.getKey());
                    p.unitUris.remove(e.getKey());
                    hadMapperScan |= old != null && old.scans.stream().anyMatch(s -> "mapperScan".equals(s.kind));
                }
            }
        }
        if (cu == null || !cu.exists()) {
            return hadMapperScan;
        }
        ProjectState p = projects.get(cu.getJavaProject().getElementName());
        if (p == null || !p.spring) {
            return hadMapperScan;
        }
        try {
            IPackageFragmentRoot root = (IPackageFragmentRoot) cu.getAncestor(IJavaElement.PACKAGE_FRAGMENT_ROOT);
            if (root != null && isTest(root)) {
                return hadMapperScan;
            }
        } catch (RuntimeException e) {
            // keep going
        }
        String text = source(cu);
        if (RELEVANT.matcher(text).find() || inMapperPackage(cu)) {
            Map<ICompilationUnit, String> texts = new HashMap<>();
            texts.put(cu, text);
            parse(p, List.of(cu), texts, monitor);
        } else {
            Facts keys = new Facts();
            propertyCalls(text, JDTUtils.toURI(cu), keys);
            if (!keys.isEmpty()) {
                p.units.put(cu.getHandleIdentifier(), keys);
                p.unitUris.put(cu.getHandleIdentifier(), JDTUtils.toURI(cu));
            }
        }
        Facts now = p.units.get(cu.getHandleIdentifier());
        boolean hasMapperScan = now != null && now.scans.stream().anyMatch(s -> "mapperScan".equals(s.kind));
        return hasMapperScan != hadMapperScan;
    }

    private boolean inMapperPackage(ICompilationUnit cu) {
        IJavaElement pkg = cu.getParent();
        String name = pkg == null ? "" : pkg.getElementName();
        for (ProjectState p : projects.values()) {
            for (Facts f : facts(p)) {
                for (ScanSpec s : f.scans) {
                    if ("mapperScan".equals(s.kind) && Resolver.inPackages(name, s.packages)) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    static String path(String uri) {
        try {
            if (uri.startsWith("file:")) {
                return Path.of(URI.create(uri)).toAbsolutePath().normalize().toString();
            }
        } catch (RuntimeException e) {
            // not a file URI
        }
        return uri;
    }

    // ---- resolution

    private static List<Facts> facts(ProjectState p) {
        List<Facts> out = new ArrayList<>(p.units.values());
        out.add(p.binary);
        return out;
    }

    private void resolve() {
        Map<String, Resolver.ProjectInput> inputs = new LinkedHashMap<>();
        for (ProjectState p : projects.values()) {
            Resolver.ProjectInput in = new Resolver.ProjectInput();
            in.name = p.name;
            in.module = p.spec == null ? null : p.spec.moduleId;
            in.dependencies.addAll(p.dependencies);
            in.facts.addAll(facts(p));
            if (p.spring) {
                in.library.addAll(library(p));
                in.activeProfiles = activeProfiles(p);
            }
            inputs.put(p.name, in);
        }
        result = new Resolver(inputs, oracle()).resolve();
    }

    private Resolver.Oracle oracle() {
        return new Resolver.Oracle() {
            @Override
            public Facts xml(String location, Collection<String> names) {
                return xmlFile(location, names);
            }

            @Override
            public boolean proxyInterface(String project, String type) {
                IType t = findType(project, type);
                try {
                    if (t == null || !t.isInterface()) {
                        return false;
                    }
                    for (IAnnotation a : t.getAnnotations()) {
                        if (!a.getElementName().endsWith("FunctionalInterface")) {
                            return true;
                        }
                    }
                } catch (JavaModelException e) {
                    return true;
                }
                return false;
            }

            @Override
            public boolean libraryComponent(String project, String type) {
                IType t = findType(project, type);
                try {
                    if (t == null || !t.isBinary()) {
                        return false;
                    }
                    for (IAnnotation a : t.getAnnotations()) {
                        if (STEREOTYPES.contains(a.getElementName())) {
                            return true;
                        }
                    }
                } catch (JavaModelException e) {
                    return true;
                }
                return false;
            }
        };
    }

    private IType findType(String project, String binaryName) {
        ProjectState p = projects.get(project);
        if (p == null || binaryName == null) {
            return null;
        }
        try {
            return p.java.findType(binaryName.replace('$', '.'));
        } catch (JavaModelException e) {
            return null;
        }
    }

    // ---- XML bean files

    /** {@code classpath:app-config.xml} on the resource folders of the projects. */
    private Facts xmlFile(String location, Collection<String> names) {
        String relative = location.replaceFirst("^classpath\\*?:", "").replaceFirst("^/+", "");
        if (relative.contains("*") || relative.contains("${")) {
            return null;
        }
        for (String name : names) {
            ProjectState p = projects.get(name);
            if (p == null) {
                continue;
            }
            for (File dir : resourceDirs(p, false)) {
                File file = new File(dir, relative);
                if (file.isFile()) {
                    return xmlFacts(p, file);
                }
            }
        }
        return null;
    }

    private Facts xmlFacts(ProjectState p, File file) {
        String key = file.getAbsolutePath();
        Object[] cached = xmlCache.get(key);
        if (cached != null && (Long) cached[0] == file.lastModified()) {
            return (Facts) cached[1];
        }
        Facts facts;
        try {
            String uri = ResourceUtils.fixURI(file.toURI());
            facts = XmlBeans.parse(Files.readString(file.toPath(), StandardCharsets.UTF_8), uri);
            // The XML file's own imports are relative to the classpath, as Spring reads them.
            List<BeanDef> typed = new ArrayList<>();
            List<IJavaElement> types = new ArrayList<>();
            for (BeanDef b : facts.beans) {
                b.project = p.name;
                IType t = b.type == null ? null : p.java.findType(b.type.replace('$', '.'));
                if (t != null) {
                    typed.add(b);
                    types.add(t);
                }
            }
            IBinding[] bindings = bindings(p.java, types.toArray(new IJavaElement[0]), null);
            for (int i = 0; i < bindings.length; i++) {
                if (bindings[i] instanceof ITypeBinding t) {
                    BeanDef b = typed.get(i);
                    TypeScanner.supertypes(t, b.assignable, b.parameterized);
                    product(t, b);
                }
            }
            for (BeanDef b : facts.beans) {
                if (b.type != null && b.assignable.isEmpty()) {
                    b.assignable.add(b.type);
                }
            }
        } catch (IOException | JavaModelException e) {
            facts = new Facts();
        }
        xmlCache.put(key, new Object[] {file.lastModified(), facts});
        return facts;
    }

    private static void product(ITypeBinding t, BeanDef b) {
        for (ITypeBinding s = t; s != null; s = s.getSuperclass()) {
            for (ITypeBinding i : s.getInterfaces()) {
                if (Annotations.FACTORY_BEAN.equals(i.getErasure().getQualifiedName()) && i.getTypeArguments().length == 1) {
                    TypeScanner.supertypes(i.getTypeArguments()[0], b.assignable, b.parameterized);
                    return;
                }
            }
        }
    }

    private static List<File> resourceDirs(ProjectState p, boolean test) {
        List<File> out = new ArrayList<>();
        if (p.spec != null) {
            out.addAll(p.spec.resourceDirs);
            if (test) {
                out.addAll(p.spec.testResourceDirs);
            }
            // Resources read from a source folder (src/main/java holding XML).
            for (ImportPlan.SourceRoot r : p.spec.sources) {
                if (!r.test || test) {
                    out.add(r.dir);
                }
            }
        }
        return out;
    }

    // ---- library beans

    /**
     * The beans libraries define: the {@code @Bean} methods of the auto-configurations the jars
     * list, of the configurations in their packages (which conditions and import selectors pick
     * among), and of the library configurations the sources import ({@code @EnableWebSecurity}).
     */
    private List<BeanDef> library(ProjectState p) {
        if (p.library != null) {
            return p.library;
        }
        List<String> jars = new ArrayList<>();
        try {
            for (IClasspathEntry e : p.java.getResolvedClasspath(true)) {
                if (e.getEntryKind() == IClasspathEntry.CPE_LIBRARY && e.getPath().toString().endsWith(".jar") && !e.isTest()) {
                    jars.add(e.getPath().toOSString());
                }
            }
        } catch (JavaModelException e) {
            p.library = List.of();
            return p.library;
        }
        Set<String> imported = new LinkedHashSet<>();
        for (String dep : closure(p.name)) {
            for (Facts f : facts(projects.get(dep))) {
                f.imports.values().forEach(imported::addAll);
            }
        }
        String key = String.join("|", jars) + "#" + String.join(",", imported);
        List<BeanDef> cached = libraryCache.get(key);
        if (cached != null) {
            p.library = cached;
            return cached;
        }
        long start = System.currentTimeMillis();
        Set<String> configs = new LinkedHashSet<>();
        for (String jar : jars) {
            configs.addAll(autoConfigurations(jar));
        }
        List<IJavaElement> types = new ArrayList<>();
        Set<String> seenTypes = new HashSet<>();
        Set<String> packages = new HashSet<>();
        try {
            for (String c : configs) {
                IType t = p.java.findType(c.replace('$', '.'));
                if (t != null && seenTypes.add(t.getFullyQualifiedName())) {
                    types.add(t);
                    IPackageFragment pkg = t.getPackageFragment();
                    if (packages.add(pkg.getHandleIdentifier())) {
                        for (IOrdinaryClassFile cf : pkg.getOrdinaryClassFiles()) {
                            IType sibling = cf.getType();
                            if (seenTypes.contains(sibling.getFullyQualifiedName())) {
                                continue;
                            }
                            for (IAnnotation a : sibling.getAnnotations()) {
                                if (a.getElementName().equals(Annotations.CONFIGURATION)
                                    || a.getElementName().equals("org.springframework.boot.autoconfigure.AutoConfiguration")) {
                                    seenTypes.add(sibling.getFullyQualifiedName());
                                    types.add(sibling);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            for (String c : imported) {
                IType t = p.java.findType(c.replace('$', '.'));
                if (t != null && t.isBinary() && seenTypes.add(t.getFullyQualifiedName())) {
                    types.add(t);
                }
            }
        } catch (JavaModelException e) {
            // what was found so far
        }
        List<BeanDef> beans = new ArrayList<>();
        Set<String> visited = new HashSet<>();
        for (IBinding b : bindings(p.java, types.toArray(new IJavaElement[0]), null)) {
            if (b instanceof ITypeBinding t) {
                libraryBeans(t, beans, visited, 0);
            }
        }
        JavaLanguageServerPlugin.logInfo("SnoutCode Spring: " + beans.size() + " library bean(s) from " + types.size()
            + " configuration(s) for " + p.name + " in " + (System.currentTimeMillis() - start) + " ms");
        libraryCache.put(key, beans);
        p.library = beans;
        return beans;
    }

    private static void libraryBeans(ITypeBinding config, List<BeanDef> out, Set<String> visited, int depth) {
        if (config == null || depth > 8 || !visited.add(config.getKey())) {
            return;
        }
        try {
            // An imported or auto-configured configuration class is a bean itself, named by its class.
            if (Annotations.meta(config.getAnnotations(), Annotations.COMPONENT) != null) {
                BeanDef self = new BeanDef();
                self.kind = "library";
                self.language = "library";
                self.type = TypeScanner.binary(config);
                self.name = Names.display(self.type);
                self.declaringType = self.type;
                TypeScanner.supertypes(config, self.assignable, self.parameterized);
                out.add(self);
            }
            for (IMethodBinding m : config.getDeclaredMethods()) {
                IAnnotationBinding bean = Annotations.direct(m.getAnnotations(), Annotations.BEAN);
                if (bean == null || m.getReturnType() == null || "void".equals(m.getReturnType().getName())) {
                    continue;
                }
                BeanDef b = new BeanDef();
                b.kind = "library";
                b.language = "library";
                List<String> names = Annotations.firstStrings(bean, "name", "value");
                names.removeIf(String::isEmpty);
                b.name = names.isEmpty() ? m.getName() : names.get(0);
                b.type = TypeScanner.binary(m.getReturnType());
                b.declaringType = TypeScanner.binary(config);
                b.method = m.getName();
                b.primary = Annotations.direct(m.getAnnotations(), Annotations.PRIMARY) != null;
                TypeScanner.supertypes(m.getReturnType(), b.assignable, b.parameterized);
                product(m.getReturnType(), b);
                out.add(b);
            }
            for (ITypeBinding member : config.getDeclaredTypes()) {
                libraryBeans(member, out, visited, depth + 1);
            }
            for (IAnnotationBinding a : Annotations.all(config.getAnnotations())) {
                if (Annotations.IMPORT.equals(Annotations.name(a))) {
                    for (ITypeBinding t : Annotations.types(Annotations.value(a, "value"))) {
                        libraryBeans(t, out, visited, depth + 1);
                    }
                }
            }
        } catch (RuntimeException e) {
            // a configuration whose classes are not on the classpath
        }
    }

    /** The auto-configurations a jar lists, in Spring Boot 2.7+'s imports file and in spring.factories. */
    private List<String> autoConfigurations(String jar) {
        Object[] cached = jarMetadata.get("auto|" + jar);
        File file = new File(jar);
        if (cached != null && (Long) cached[0] == file.lastModified()) {
            @SuppressWarnings("unchecked")
            List<String> list = (List<String>) cached[1];
            return list;
        }
        List<String> out = new ArrayList<>();
        try (ZipFile zip = new ZipFile(file)) {
            ZipEntry imports = zip.getEntry("META-INF/spring/org.springframework.boot.autoconfigure.AutoConfiguration.imports");
            if (imports != null) {
                for (String line : new String(zip.getInputStream(imports).readAllBytes(), StandardCharsets.UTF_8).split("\\R")) {
                    String l = line.strip();
                    if (!l.isEmpty() && !l.startsWith("#")) {
                        out.add(l);
                    }
                }
            }
            ZipEntry factories = zip.getEntry("META-INF/spring.factories");
            if (factories != null) {
                Properties props = new Properties();
                try (InputStream in = zip.getInputStream(factories)) {
                    props.load(new InputStreamReader(in, StandardCharsets.UTF_8));
                }
                String value = props.getProperty("org.springframework.boot.autoconfigure.EnableAutoConfiguration");
                if (value != null) {
                    for (String c : value.split(",")) {
                        if (!c.isBlank()) {
                            out.add(c.strip());
                        }
                    }
                }
            }
        } catch (IOException e) {
            // not a readable jar
        }
        jarMetadata.put("auto|" + jar, new Object[] {file.lastModified(), out});
        return out;
    }

    private Set<String> closure(String name) {
        Set<String> out = new LinkedHashSet<>();
        List<String> work = new ArrayList<>(List.of(name));
        while (!work.isEmpty()) {
            String n = work.remove(work.size() - 1);
            ProjectState p = projects.get(n);
            if (p != null && out.add(n)) {
                work.addAll(p.dependencies);
            }
        }
        return out;
    }

    // ---- queries

    private JsonObject summary() {
        JsonObject s = new JsonObject();
        int beans = 0;
        int injections = 0;
        int endpoints = 0;
        int warnings = 0;
        for (Resolver.Context c : topContexts()) {
            beans += c.beans.size();
            endpoints += c.endpoints.size();
        }
        injections = result == null ? 0 : result.injections.size();
        if (result != null) {
            for (JsonArray a : result.diagnostics.values()) {
                warnings += a.size();
            }
        }
        s.addProperty("beans", beans);
        s.addProperty("endpoints", endpoints);
        s.addProperty("injections", injections);
        s.addProperty("warnings", warnings);
        if (stats.has("units")) {
            s.add("units", stats.get("units"));
            s.add("scanMillis", stats.get("scanMillis"));
        }
        return s;
    }

    private List<Resolver.Context> topContexts() {
        List<Resolver.Context> out = new ArrayList<>();
        if (result == null) {
            return out;
        }
        Set<String> dependedOn = new HashSet<>();
        projects.values().forEach(p -> dependedOn.addAll(p.dependencies));
        for (Resolver.Context c : result.contexts.values()) {
            ProjectState p = projects.get(c.root);
            if (p != null && p.spring && !dependedOn.contains(c.root)) {
                out.add(c);
            }
        }
        return out;
    }

    /** The modules asked for: one by its module id (or project name), else every module with Spring. */
    private List<ProjectState> modules(String module) throws Exception {
        List<ProjectState> out = new ArrayList<>();
        for (ProjectState p : projects.values()) {
            if (!p.spring) {
                continue;
            }
            String id = p.spec != null && p.spec.moduleId != null ? p.spec.moduleId : null;
            if (module == null ? id != null || p.spec == null : module.equals(id) || module.equals(p.name)) {
                out.add(p);
            }
        }
        if (module != null && out.isEmpty()) {
            throw new IllegalArgumentException("no Spring module " + module);
        }
        return out;
    }

    private static String moduleOf(ProjectState p) {
        return p.spec != null && p.spec.moduleId != null ? p.spec.moduleId : p.name;
    }

    synchronized JsonObject beans(String module, IProgressMonitor monitor) throws Exception {
        ensureScanned(monitor);
        JsonArray modules = new JsonArray();
        for (ProjectState p : modules(module)) {
            Resolver.Context c = result.contexts.get(p.name);
            if (c == null || (module == null && c.beans.isEmpty())) {
                continue;
            }
            JsonObject m = new JsonObject();
            m.addProperty("module", moduleOf(p));
            m.addProperty("project", p.name);
            JsonArray apps = new JsonArray();
            for (ScanSpec s : c.applications) {
                JsonObject a = new JsonObject();
                a.addProperty("class", Names.display(s.declaringType));
                a.add("scanPackages", Facts.strings(s.packages));
                if (s.loc != null) {
                    a.addProperty("uri", s.loc.uri);
                    a.add("range", s.loc.range());
                }
                apps.add(a);
            }
            m.add("applications", apps);
            m.add("scanPackages", Facts.strings(c.scanAll ? List.of() : c.scanPackages));
            m.addProperty("scansEverything", c.scanAll);
            m.add("xmlFiles", Facts.strings(c.xmlFiles));
            JsonArray beans = new JsonArray();
            List<BeanDef> sorted = new ArrayList<>(c.beans);
            sorted.sort(Comparator.comparing((BeanDef b) -> b.name == null ? "" : b.name));
            for (BeanDef b : sorted) {
                JsonObject o = b.toJson();
                o.addProperty("injectedInto", result.usages.getOrDefault(b.id(), List.of()).size());
                if (!c.active(b)) {
                    o.addProperty("active", false);
                }
                beans.add(o);
            }
            m.add("beans", beans);
            m.addProperty("libraryBeans", c.library.size());
            if (c.activeProfiles != null) {
                m.add("activeProfiles", Facts.strings(new ArrayList<>(c.activeProfiles)));
            }
            modules.add(m);
        }
        JsonObject out = new JsonObject();
        out.add("modules", modules);
        return out;
    }

    synchronized JsonObject endpoints(String module, IProgressMonitor monitor) throws Exception {
        ensureScanned(monitor);
        JsonArray endpoints = new JsonArray();
        for (ProjectState p : modules(module)) {
            Resolver.Context c = result.contexts.get(p.name);
            if (c == null) {
                continue;
            }
            List<Endpoint> sorted = new ArrayList<>(c.endpoints);
            sorted.sort(Comparator.comparing((Endpoint e) -> e.paths.isEmpty() ? "" : e.paths.get(0))
                .thenComparing(e -> String.join(",", e.methods)));
            for (Endpoint e : sorted) {
                endpoints.add(e.toJson(moduleOf(p)));
            }
        }
        JsonObject out = new JsonObject();
        out.add("endpoints", endpoints);
        return out;
    }

    /** Per line of a file: its injection points with the beans they get, and its beans with where they go. */
    synchronized JsonObject injections(String uri, IProgressMonitor monitor) throws Exception {
        ensureScanned(monitor);
        String path = path(uri);
        Map<Integer, JsonObject> lines = new java.util.TreeMap<>();
        for (InjectionPoint i : result.injections) {
            if (i.loc == null || !path(i.loc.uri).equals(path)) {
                continue;
            }
            line(lines, i.loc.line).getAsJsonArray("injections").add(injectionJson(i));
        }
        Set<String> seen = new HashSet<>();
        for (Resolver.Context c : result.contexts.values()) {
            for (BeanDef b : c.beans) {
                if (b.loc == null || !path(b.loc.uri).equals(path) || !seen.add(b.id())) {
                    continue;
                }
                JsonObject o = b.toJson();
                if (inactiveEverywhere(b)) {
                    o.addProperty("active", false);
                }
                JsonArray into = new JsonArray();
                for (InjectionPoint i : result.usages.getOrDefault(b.id(), List.of())) {
                    JsonObject site = new JsonObject();
                    if (i.loc != null) {
                        site.addProperty("uri", i.loc.uri);
                        site.add("range", i.loc.range());
                    }
                    site.addProperty("declaringType", Names.display(i.declaringType));
                    site.addProperty("kind", i.kind);
                    if (i.member != null) {
                        site.addProperty("member", i.member);
                    }
                    if (i.name != null) {
                        site.addProperty("name", i.name);
                    }
                    into.add(site);
                }
                o.add("injectedInto", into);
                line(lines, b.loc.line).getAsJsonArray("beans").add(o);
            }
        }
        JsonArray out = new JsonArray();
        lines.values().forEach(out::add);
        JsonObject result = new JsonObject();
        result.addProperty("uri", uri);
        result.add("lines", out);
        return result;
    }

    private static JsonObject line(Map<Integer, JsonObject> lines, int line) {
        return lines.computeIfAbsent(line, l -> {
            JsonObject o = new JsonObject();
            o.addProperty("line", l);
            o.add("injections", new JsonArray());
            o.add("beans", new JsonArray());
            return o;
        });
    }

    private static JsonObject injectionJson(InjectionPoint i) {
        JsonObject o = new JsonObject();
        o.addProperty("kind", i.kind);
        o.addProperty("annotation", i.annotation);
        o.addProperty("declaringType", Names.display(i.declaringType));
        if (i.member != null) {
            o.addProperty("member", i.member);
        }
        if (i.name != null) {
            o.addProperty("name", i.name);
        }
        if (i.type != null) {
            o.addProperty("type", i.typeDisplay());
        }
        if (i.wrapper != null) {
            o.addProperty("wrapper", i.wrapper);
        }
        o.addProperty("required", i.required);
        if (!i.qualifiers.isEmpty()) {
            o.add("qualifiers", Facts.strings(i.qualifiers));
        }
        if (i.value != null) {
            o.addProperty("value", i.value);
        }
        o.addProperty("status", i.status);
        if (i.message != null) {
            o.addProperty("message", i.message);
        }
        o.add("range", i.loc.range());
        JsonArray beans = new JsonArray();
        for (BeanDef b : i.beans) {
            JsonObject r = new JsonObject();
            r.addProperty("name", b.name);
            r.addProperty("type", Names.display(b.type));
            r.addProperty("kind", b.kind);
            if (b.declaringType != null) {
                r.addProperty("declaringType", Names.display(b.declaringType));
            }
            if (b.method != null) {
                r.addProperty("method", b.method);
            }
            if (b.loc != null) {
                r.addProperty("uri", b.loc.uri);
                r.add("range", b.loc.range());
            }
            beans.add(r);
        }
        o.add("beans", beans);
        return o;
    }

    private static JsonObject toJson(Map<String, JsonArray> diagnostics) {
        JsonObject o = new JsonObject();
        diagnostics.forEach(o::add);
        return o;
    }

    // ---- properties files

    synchronized ProjectState projectForFile(String uri) {
        String path = path(uri);
        ProjectState best = null;
        int bestDepth = -1;
        for (ProjectState p : projects.values()) {
            if (!p.spring || p.spec == null) {
                continue;
            }
            List<File> dirs = new ArrayList<>(resourceDirs(p, true));
            dirs.add(p.spec.projectDir);
            for (File dir : dirs) {
                String d = dir.toPath().toAbsolutePath().normalize().toString();
                if (path.startsWith(d + File.separator)) {
                    int depth = d.length() + (dir.equals(p.spec.projectDir) ? 0 : 10_000);
                    if (depth > bestDepth) {
                        best = p;
                        bestDepth = depth;
                    }
                }
            }
        }
        if (best == null) {
            best = projects.values().stream().filter(p -> p.spring).findFirst().orElse(null);
        }
        return best;
    }

    /** The properties service of the project that owns a properties file, with the placeholders of its text as known keys. */
    synchronized PropertiesService properties(String uri, String text, IProgressMonitor monitor) throws Exception {
        ensureScanned(monitor);
        ProjectState p = projectForFile(uri);
        ConfigMetadata meta = new ConfigMetadata();
        Map<String, List<KeyUse>> used = new LinkedHashMap<>();
        if (p == null) {
            return new PropertiesService(meta, used, t -> null);
        }
        Set<String> closure = closure(p.name);
        for (String name : closure) {
            for (Facts f : facts(projects.get(name))) {
                for (Facts.SourceProperty sp : f.properties) {
                    ConfigMetadata.Property prop = new ConfigMetadata.Property();
                    prop.name = sp.name;
                    prop.type = sp.type;
                    prop.description = sp.description;
                    prop.defaultValue = sp.defaultValue;
                    prop.sourceType = sp.sourceType;
                    prop.deprecated = sp.deprecated;
                    prop.loc = sp.loc;
                    prop.origin = "source";
                    meta.add(prop);
                }
                for (KeyUse k : f.keyUses) {
                    used.computeIfAbsent(Names.relaxed(k.key), x -> new ArrayList<>()).add(k);
                }
            }
        }
        for (Object[] cached : xmlCache.values()) {
            for (KeyUse k : ((Facts) cached[1]).keyUses) {
                used.computeIfAbsent(Names.relaxed(k.key), x -> new ArrayList<>()).add(k);
            }
        }
        if (text != null) {
            for (Placeholders.Key k : Placeholders.keys(text)) {
                KeyUse use = new KeyUse();
                use.key = k.key();
                used.computeIfAbsent(Names.relaxed(k.key()), x -> new ArrayList<>()).add(use);
            }
        }
        for (File dir : resourceDirs(p, true)) {
            loggingKeys(dir, used);
            File additional = new File(dir, "META-INF/additional-spring-configuration-metadata.json");
            if (additional.isFile()) {
                try {
                    meta.read(JsonParser.parseString(Files.readString(additional.toPath())).getAsJsonObject(), additional.getPath());
                } catch (IOException | RuntimeException e) {
                    // a broken file adds nothing
                }
            }
        }
        for (IClasspathEntry e : p.java.getResolvedClasspath(true)) {
            if (e.getEntryKind() == IClasspathEntry.CPE_LIBRARY && e.getPath().toString().endsWith(".jar")) {
                for (JsonObject json : jarMetadata(e.getPath().toOSString())) {
                    meta.read(json, e.getPath().toOSString());
                }
            }
        }
        ProjectState owner = p;
        PropertiesService service = new PropertiesService(meta, used, type -> enumConstants(owner, type));
        Profiles.Config config = profileConfig(p);
        for (BeanDef b : beansOf(p)) {
            for (String expr : b.profiles) {
                for (String n : Profiles.names(expr)) {
                    config.know(n, "@Profile on " + Names.display(b.method != null ? b.declaringType : b.type));
                }
            }
        }
        service.knownProfiles = config.known;
        service.activeProfiles = activeProfiles(p);
        service.fileProfile = Profiles.fileProfile(path(uri));
        return service;
    }

    private static final Pattern SPRING_PROPERTY = Pattern.compile("<springProperty\\b[^>]*?\\bsource\\s*=\\s*\"([^\"]+)\"");

    /** Keys Logback reads through {@code <springProperty source="...">}. */
    private static void loggingKeys(File dir, Map<String, List<KeyUse>> used) {
        for (String name : List.of("logback-spring.xml", "logback.xml")) {
            File f = new File(dir, name);
            if (!f.isFile()) {
                continue;
            }
            try {
                String text = Files.readString(f.toPath(), StandardCharsets.UTF_8);
                LineIndex lines = new LineIndex(text);
                Matcher m = SPRING_PROPERTY.matcher(text);
                while (m.find()) {
                    KeyUse k = new KeyUse();
                    k.key = m.group(1);
                    k.loc = Loc.of(ResourceUtils.fixURI(f.toURI()), lines, m.start(1), m.group(1).length());
                    used.computeIfAbsent(Names.relaxed(k.key), x -> new ArrayList<>()).add(k);
                }
            } catch (IOException e) {
                // unreadable: no keys
            }
        }
    }

    private List<JsonObject> jarMetadata(String jar) {
        File file = new File(jar);
        Object[] cached = jarMetadata.get("meta|" + jar);
        if (cached != null && (Long) cached[0] == file.lastModified()) {
            @SuppressWarnings("unchecked")
            List<JsonObject> list = (List<JsonObject>) cached[1];
            return list;
        }
        List<JsonObject> out = new ArrayList<>();
        try (ZipFile zip = new ZipFile(file)) {
            for (String name : List.of("META-INF/spring-configuration-metadata.json", "META-INF/additional-spring-configuration-metadata.json")) {
                ZipEntry entry = zip.getEntry(name);
                if (entry != null) {
                    try (InputStream in = zip.getInputStream(entry)) {
                        JsonElement json = JsonParser.parseString(new String(in.readAllBytes(), StandardCharsets.UTF_8));
                        if (json.isJsonObject()) {
                            out.add(json.getAsJsonObject());
                        }
                    }
                }
            }
        } catch (IOException | RuntimeException e) {
            // not a readable jar
        }
        jarMetadata.put("meta|" + jar, new Object[] {file.lastModified(), out});
        return out;
    }

    private List<String> enumConstants(ProjectState p, String type) {
        if (type == null || type.isEmpty() || type.contains("<") || PropertiesService.isScalar(type) && type.startsWith("java.")) {
            return null;
        }
        String key = p.name + "|" + type;
        if (enumCache.containsKey(key)) {
            return enumCache.get(key);
        }
        List<String> out = null;
        try {
            IType t = p.java.findType(type.replace('$', '.'));
            if (t != null && t.isEnum()) {
                out = new ArrayList<>();
                for (IField f : t.getFields()) {
                    if (f.isEnumConstant()) {
                        out.add(f.getElementName());
                    }
                }
            }
        } catch (JavaModelException e) {
            out = null;
        }
        enumCache.put(key, out);
        return out;
    }

    /** Where a key in a properties file is defined: the property's field or setter, its class, or the code that reads it. */
    synchronized JsonArray definition(String uri, PropertiesService service, String key) throws JavaModelException {
        JsonArray out = new JsonArray();
        PropertiesService.Match m = service.find(key);
        if (m == null) {
            return out;
        }
        ProjectState p = projectForFile(uri);
        if (m.property != null) {
            if (m.property.loc != null) {
                out.add(m.property.loc.location());
            } else if (m.property.sourceType != null && p != null) {
                IType t = p.java.findType(m.property.sourceType.replace('$', '.'));
                if (t != null) {
                    String last = m.property.name.substring(m.property.name.lastIndexOf('.') + 1);
                    String camel = Names.camel(last);
                    IJavaElement target = t;
                    IField f = t.getField(camel);
                    if (f != null && f.exists()) {
                        target = f;
                    } else {
                        for (IMethod method : t.getMethods()) {
                            String n = method.getElementName();
                            if (n.equalsIgnoreCase("set" + camel) || n.equalsIgnoreCase("get" + camel) || n.equalsIgnoreCase(camel)) {
                                target = method;
                                break;
                            }
                        }
                    }
                    Loc loc = Locations.toLoc(JDTUtils.toLocation(target));
                    if (loc != null) {
                        out.add(loc.location());
                    }
                }
            }
        }
        for (KeyUse u : m.uses) {
            if (u.loc != null) {
                out.add(u.loc.location());
            }
        }
        return out;
    }

    // ---- @Value inlay hints

    synchronized JsonArray inlayHints(String uri, int fromLine, int toLine, IProgressMonitor monitor) throws Exception {
        ensureScanned(monitor);
        JsonArray out = new JsonArray();
        String path = path(uri);
        for (ProjectState p : projects.values()) {
            for (Map.Entry<String, String> e : p.unitUris.entrySet()) {
                if (!path(e.getValue()).equals(path)) {
                    continue;
                }
                Facts f = p.units.get(e.getKey());
                if (f == null) {
                    continue;
                }
                Map<String, String> values = null;
                for (KeyUse k : f.keyUses) {
                    if (k.literal == null || k.literal.line < fromLine || k.literal.line > toLine) {
                        continue;
                    }
                    if (values == null) {
                        values = propertyValues(p);
                    }
                    String value = values.get(Names.relaxed(k.key));
                    String label;
                    if (value != null) {
                        label = "= " + shorten(value);
                    } else if (k.defaultValue != null) {
                        label = "= " + shorten(k.defaultValue) + " (default)";
                    } else {
                        continue;
                    }
                    JsonObject hint = new JsonObject();
                    hint.add("position", Loc.position(k.literal.endLine, k.literal.endCharacter));
                    hint.addProperty("label", label);
                    hint.addProperty("paddingLeft", true);
                    JsonObject tooltip = new JsonObject();
                    tooltip.addProperty("kind", "markdown");
                    tooltip.addProperty("value", "`" + k.key + "`" + (value != null ? " from the application properties" : " is not set"));
                    hint.add("tooltip", tooltip);
                    out.add(hint);
                }
            }
        }
        return out;
    }

    private static String shorten(String value) {
        String v = value.replace("\n", " ");
        return v.length() > 40 ? v.substring(0, 39) + "…" : v;
    }

    /** The values of application.properties and .yml, with those of the active profiles over them. */
    private Map<String, String> propertyValues(ProjectState p) {
        Set<String> evaluated = activeProfiles(p);
        if (evaluated != null) {
            return Profiles.values(profileConfig(p), evaluated);
        }
        Map<String, String> values = new HashMap<>();
        List<File> dirs = resourceDirs(p, false);
        read(dirs, "application", values);
        String active = values.get(Names.relaxed("spring.profiles.active"));
        if (active != null) {
            for (String profile : active.split(",")) {
                if (!profile.isBlank()) {
                    read(dirs, "application-" + profile.strip(), values);
                }
            }
        }
        return values;
    }

    private static void read(List<File> dirs, String base, Map<String, String> values) {
        for (File dir : dirs) {
            for (String ext : List.of(".properties", ".yml", ".yaml")) {
                File f = new File(dir, base + ext);
                if (!f.isFile()) {
                    continue;
                }
                try {
                    PropsDoc doc = PropsDoc.parse(Files.readString(f.toPath(), StandardCharsets.UTF_8), !ext.equals(".properties"));
                    for (PropsDoc.Entry e : doc.entries) {
                        if (e.value != null) {
                            values.put(Names.relaxed(e.key), e.value);
                        }
                    }
                } catch (IOException e) {
                    // unreadable: skip
                }
            }
        }
    }
}
