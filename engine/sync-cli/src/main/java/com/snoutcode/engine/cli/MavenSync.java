package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import org.apache.maven.model.Build;
import org.apache.maven.model.Model;
import org.apache.maven.model.Plugin;
import org.apache.maven.model.PluginExecution;
import org.apache.maven.model.Profile;
import org.apache.maven.model.Resource;
import org.apache.maven.model.building.DefaultModelBuilderFactory;
import org.apache.maven.model.building.DefaultModelBuildingRequest;
import org.apache.maven.model.building.FileModelSource;
import org.apache.maven.model.building.ModelBuilder;
import org.apache.maven.model.building.ModelBuildingException;
import org.apache.maven.model.building.ModelBuildingRequest;
import org.apache.maven.model.building.ModelBuildingResult;
import org.apache.maven.model.building.ModelProblem;
import org.apache.maven.model.building.ModelSource;
import org.apache.maven.model.resolution.InvalidRepositoryException;
import org.apache.maven.model.resolution.ModelResolver;
import org.apache.maven.model.resolution.UnresolvableModelException;
import org.codehaus.plexus.util.xml.Xpp3Dom;
import org.eclipse.aether.DefaultRepositorySystemSession;
import org.eclipse.aether.RepositorySystem;
import org.eclipse.aether.RepositorySystemSession;
import org.eclipse.aether.artifact.Artifact;
import org.eclipse.aether.artifact.ArtifactProperties;
import org.eclipse.aether.artifact.ArtifactType;
import org.eclipse.aether.artifact.ArtifactTypeRegistry;
import org.eclipse.aether.artifact.DefaultArtifact;
import org.eclipse.aether.artifact.DefaultArtifactType;
import org.eclipse.aether.collection.CollectRequest;
import org.eclipse.aether.graph.Dependency;
import org.eclipse.aether.graph.DependencyNode;
import org.eclipse.aether.graph.Exclusion;
import org.eclipse.aether.repository.RemoteRepository;
import org.eclipse.aether.repository.RepositoryPolicy;
import org.eclipse.aether.repository.WorkspaceReader;
import org.eclipse.aether.repository.WorkspaceRepository;
import org.eclipse.aether.resolution.ArtifactRequest;
import org.eclipse.aether.resolution.ArtifactResolutionException;
import org.eclipse.aether.resolution.ArtifactResult;
import org.eclipse.aether.resolution.DependencyRequest;
import org.eclipse.aether.resolution.DependencyResolutionException;
import org.eclipse.aether.resolution.DependencyResult;
import org.eclipse.aether.resolution.VersionRangeRequest;
import org.eclipse.aether.resolution.VersionRangeResolutionException;
import org.eclipse.aether.resolution.VersionRangeResult;
import org.eclipse.aether.util.graph.manager.DependencyManagerUtils;
import org.eclipse.aether.util.graph.visitor.PreorderNodeListGenerator;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Deque;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * Syncs one Maven build (a reactor: the root POM and every module it lists, in any profile that
 * is active) into the same entry a Gradle build gets, so the rest of the engine, the importer
 * and the app treat both alike. Nothing runs: the effective POMs come from Maven's model builder
 * (parents from the folder or the repositories, BOM imports, profiles, interpolation), and the
 * classpaths from Maven's resolver with Maven's own dependency mediation and scopes.
 *
 * <p>Each module becomes a project with a {@code main} and a {@code test} source set:
 *
 * <ul>
 *   <li>sources: the build's source folders, the build helper's added ones, and the generated
 *       ones (annotation processors' {@code target/generated-sources/annotations}, and every
 *       folder already under {@code target/generated-sources});</li>
 *   <li>classpaths: {@code main} compiles against {@code compile}, {@code provided} and
 *       {@code system} dependencies and runs with {@code compile} and {@code runtime} ones;
 *       {@code test} has them all. Other modules of the reactor are project references;</li>
 *   <li>Java: the compiler plugin's {@code release}, {@code source} and {@code target}, else the
 *       {@code maven.compiler.*} properties; a toolchain's JDK when the toolchains plugin or the
 *       compiler's {@code jdkToolchain} asks for one.</li>
 * </ul>
 *
 * Under {@code maven} each project also carries what the Maven window shows: its coordinates,
 * packaging, modules, profiles (and which are active) and plugins.
 */
final class MavenSync {
    /** The build's own options. */
    static final class Options {
        File rootDir;
        /** The JDK the build runs on, and compiles with unless a toolchain says otherwise. */
        File javaHome;
        List<JavaHomes.Jdk> jdks = List.of();
        boolean offline;
        boolean sources = true;
        /** Profiles to activate, and with a leading {@code !} to deactivate, on top of the POMs'. */
        List<String> profiles = List.of();
        /** {@code -D} properties for the build. */
        Map<String, String> properties = Map.of();
    }

    static final String CENTRAL = "https://repo.maven.apache.org/maven2";
    static final String COMPILER = "org.apache.maven.plugins:maven-compiler-plugin";
    static final String SPRING_BOOT = "org.springframework.boot:spring-boot-maven-plugin";
    private static final Pattern MODULE = Pattern.compile("<module>\\s*([^<]+?)\\s*</module>");
    private static final Pattern PROPERTY = Pattern.compile("\\$\\{([^}]+)}");

    private MavenSync() {
    }

    // ---- the build: a reactor of modules

    /** A module of the reactor, once its effective model is built. */
    static final class Module {
        File pom;
        Model model;
        ModelBuildingResult result;
        String path;
        final List<String> problems = new ArrayList<>();
    }

    /** Reads the reactor and resolves every module; returns the build's entry (the {@code builds} part). */
    static JsonObject run(Options options) throws IOException {
        long start = System.nanoTime();
        File root = options.rootDir.getCanonicalFile();
        MavenEnv env = MavenEnv.load(options.offline);
        Properties user = userProperties(root, options);
        List<String> active = new ArrayList<>(env.activeSettingsProfiles());
        List<String> inactive = new ArrayList<>();
        for (String p : profiles(root, options)) {
            if (p.startsWith("!") || p.startsWith("-")) {
                inactive.add(p.substring(1));
            } else if (p.startsWith("+")) {
                active.add(p.substring(1));
            } else {
                active.add(p);
            }
        }
        DefaultRepositorySystemSession session = env.session(user);
        Reactor reactor = new Reactor();
        reactor.scan(root);
        session.setWorkspaceReader(reactor);
        RepositorySystem system = MavenEnv.system();
        ModelBuilder builder = new DefaultModelBuilderFactory().newInstance();
        List<RemoteRepository> base = system.newResolutionRepositories(session, List.of(central()));
        List<org.apache.maven.model.Profile> settingsProfiles = settingsProfiles(env);

        // Build every module's effective model, from the root down its <modules>.
        List<Module> modules = new ArrayList<>();
        List<String> errors = new ArrayList<>();
        Deque<File> queue = new ArrayDeque<>();
        Set<String> seen = new LinkedHashSet<>();
        queue.add(new File(root, "pom.xml"));
        while (!queue.isEmpty()) {
            File pom = queue.removeFirst().getCanonicalFile();
            if (!seen.add(pom.getPath()) || !pom.isFile()) {
                continue;
            }
            Module m = new Module();
            m.pom = pom;
            DefaultModelBuildingRequest request = new DefaultModelBuildingRequest();
            request.setPomFile(pom);
            request.setValidationLevel(ModelBuildingRequest.VALIDATION_LEVEL_MINIMAL);
            request.setProcessPlugins(true);
            request.setTwoPhaseBuilding(false);
            request.setLocationTracking(true);
            request.setSystemProperties(env.systemProperties());
            request.setUserProperties(user);
            request.setActiveProfileIds(active);
            request.setInactiveProfileIds(inactive);
            request.setProfiles(settingsProfiles);
            request.setModelResolver(new Resolver(system, session, base, reactor));
            try {
                m.result = builder.build(request);
                m.model = m.result.getEffectiveModel();
                for (ModelProblem p : m.result.getProblems()) {
                    if (p.getSeverity() != ModelProblem.Severity.WARNING) {
                        m.problems.add(describe(p));
                    }
                }
            } catch (ModelBuildingException e) {
                List<String> problems = new ArrayList<>();
                for (ModelProblem p : e.getProblems()) {
                    if (p.getSeverity() != ModelProblem.Severity.WARNING) {
                        problems.add(describe(p));
                    }
                }
                if (pom.equals(new File(root, "pom.xml").getCanonicalFile())) {
                    throw new IOException("The POM cannot be read: " + String.join("; ", problems), e);
                }
                errors.add(pom + ": " + String.join("; ", problems));
                continue;
            }
            m.path = modules.isEmpty() ? ":" : ":" + m.model.getArtifactId();
            modules.add(m);
            reactor.add(m.model, pom);
            for (String child : m.model.getModules()) {
                File f = new File(pom.getParentFile(), child);
                queue.add(f.isDirectory() ? new File(f, "pom.xml") : f);
            }
        }

        List<File> poms = new ArrayList<>();
        modules.forEach(m -> poms.add(m.pom));
        JsonArray projects = new JsonArray();
        for (Module m : modules) {
            projects.add(project(m, root, options, env, session, system, reactor, poms, errors));
        }
        JsonObject build = new JsonObject();
        build.addProperty("name", modules.get(0).model.getArtifactId());
        build.addProperty("kind", "root");
        build.addProperty("rootDir", root.getAbsolutePath());
        build.add("projects", projects);
        JsonArray errs = new JsonArray();
        errors.forEach(errs::add);
        build.add("errors", errs);

        JsonObject out = new JsonObject();
        out.addProperty("tool", "maven");
        out.addProperty("rootDir", root.getAbsolutePath());
        out.addProperty("mavenVersion", MavenHome.wrapperVersion(root));
        out.addProperty("gradleVersion", (String) null);
        out.addProperty("gradleJavaHome", options.javaHome == null ? null : options.javaHome.getAbsolutePath());
        out.addProperty("localRepository", env.localRepository.toString());
        JsonArray builds = new JsonArray();
        builds.add(build);
        out.add("builds", builds);
        out.addProperty("syncMillis", (System.nanoTime() - start) / 1_000_000);
        return out;
    }

    private static String describe(ModelProblem p) {
        String where = p.getSource() == null || p.getSource().isEmpty() ? "" : new File(p.getSource()).getName()
            + (p.getLineNumber() > 0 ? ":" + p.getLineNumber() : "") + ": ";
        return where + p.getMessage();
    }

    static RemoteRepository central() {
        return new RemoteRepository.Builder("central", "default", CENTRAL)
            .setSnapshotPolicy(new RepositoryPolicy(false, RepositoryPolicy.UPDATE_POLICY_DAILY, RepositoryPolicy.CHECKSUM_POLICY_WARN))
            .build();
    }

    /** The {@code -D} properties: {@code .mvn/maven.config}'s, then the options'. */
    static Properties userProperties(File root, Options options) {
        Properties out = new Properties();
        for (String arg : mavenConfig(root)) {
            if (arg.startsWith("-D") && arg.length() > 2) {
                String[] kv = arg.substring(2).split("=", 2);
                out.setProperty(kv[0], kv.length > 1 ? kv[1] : "true");
            }
        }
        options.properties.forEach(out::setProperty);
        return out;
    }

    /** The profiles {@code .mvn/maven.config} and the options name ({@code -P a,!b}). */
    static List<String> profiles(File root, Options options) {
        List<String> out = new ArrayList<>();
        List<String> args = mavenConfig(root);
        for (int i = 0; i < args.size(); i++) {
            String a = args.get(i);
            String list = null;
            if ((a.equals("-P") || a.equals("--activate-profiles")) && i + 1 < args.size()) {
                list = args.get(++i);
            } else if (a.startsWith("-P") && a.length() > 2) {
                list = a.substring(2);
            }
            if (list != null) {
                for (String p : list.split(",")) {
                    if (!p.isBlank()) {
                        out.add(p.trim());
                    }
                }
            }
        }
        out.addAll(options.profiles);
        return out;
    }

    /** The arguments in {@code .mvn/maven.config}, one or more per line. */
    static List<String> mavenConfig(File root) {
        Path config = root.toPath().resolve(".mvn/maven.config");
        List<String> out = new ArrayList<>();
        if (!Files.isRegularFile(config)) {
            return out;
        }
        try {
            for (String line : Files.readAllLines(config, StandardCharsets.UTF_8)) {
                for (String t : line.trim().split("\\s+")) {
                    if (!t.isEmpty()) {
                        out.add(t);
                    }
                }
            }
        } catch (IOException e) {
            // no config
        }
        return out;
    }

    private static List<org.apache.maven.model.Profile> settingsProfiles(MavenEnv env) {
        List<org.apache.maven.model.Profile> out = new ArrayList<>();
        for (org.apache.maven.settings.Profile s : env.settings.getProfiles()) {
            org.apache.maven.model.Profile p = new org.apache.maven.model.Profile();
            p.setId(s.getId());
            p.setSource("settings.xml");
            if (s.getActivation() != null) {
                org.apache.maven.model.Activation a = new org.apache.maven.model.Activation();
                a.setActiveByDefault(s.getActivation().isActiveByDefault());
                a.setJdk(s.getActivation().getJdk());
                if (s.getActivation().getProperty() != null) {
                    org.apache.maven.model.ActivationProperty ap = new org.apache.maven.model.ActivationProperty();
                    ap.setName(s.getActivation().getProperty().getName());
                    ap.setValue(s.getActivation().getProperty().getValue());
                    a.setProperty(ap);
                }
                p.setActivation(a);
            }
            p.setProperties(s.getProperties());
            for (org.apache.maven.settings.Repository r : s.getRepositories()) {
                p.addRepository(repository(r));
            }
            for (org.apache.maven.settings.Repository r : s.getPluginRepositories()) {
                p.addPluginRepository(repository(r));
            }
            out.add(p);
        }
        return out;
    }

    private static org.apache.maven.model.Repository repository(org.apache.maven.settings.Repository r) {
        org.apache.maven.model.Repository out = new org.apache.maven.model.Repository();
        out.setId(r.getId());
        out.setUrl(r.getUrl());
        out.setLayout(r.getLayout());
        if (r.getReleases() != null) {
            org.apache.maven.model.RepositoryPolicy p = new org.apache.maven.model.RepositoryPolicy();
            p.setEnabled(r.getReleases().isEnabled());
            p.setUpdatePolicy(r.getReleases().getUpdatePolicy());
            p.setChecksumPolicy(r.getReleases().getChecksumPolicy());
            out.setReleases(p);
        }
        if (r.getSnapshots() != null) {
            org.apache.maven.model.RepositoryPolicy p = new org.apache.maven.model.RepositoryPolicy();
            p.setEnabled(r.getSnapshots().isEnabled());
            p.setUpdatePolicy(r.getSnapshots().getUpdatePolicy());
            p.setChecksumPolicy(r.getSnapshots().getChecksumPolicy());
            out.setSnapshots(p);
        }
        return out;
    }

    static RemoteRepository remote(org.apache.maven.model.Repository r) {
        RemoteRepository.Builder b = new RemoteRepository.Builder(r.getId(), r.getLayout() == null ? "default" : r.getLayout(), r.getUrl());
        if (r.getReleases() != null) {
            b.setReleasePolicy(policy(r.getReleases()));
        }
        if (r.getSnapshots() != null) {
            b.setSnapshotPolicy(policy(r.getSnapshots()));
        }
        return b.build();
    }

    /**
     * A POM's repository policy for the resolver, with Maven's defaults for what it leaves out
     * (a daily update, a warning on a bad checksum): the resolver takes a missing policy as an
     * empty one and fails on it ("Unsupported policy: ").
     */
    static RepositoryPolicy policy(org.apache.maven.model.RepositoryPolicy p) {
        String update = p.getUpdatePolicy() == null || p.getUpdatePolicy().isBlank()
            ? RepositoryPolicy.UPDATE_POLICY_DAILY : p.getUpdatePolicy().trim();
        String checksum = p.getChecksumPolicy() == null || p.getChecksumPolicy().isBlank()
            ? RepositoryPolicy.CHECKSUM_POLICY_WARN : p.getChecksumPolicy().trim();
        return new RepositoryPolicy(p.isEnabled(), update, checksum);
    }

    // ---- one module

    private static JsonObject project(Module m, File root, Options options, MavenEnv env, RepositorySystemSession session,
                                      RepositorySystem system, Reactor reactor, List<File> poms, List<String> buildErrors) {
        Model model = m.model;
        Build build = model.getBuild() == null ? new Build() : model.getBuild();
        File dir = m.pom.getParentFile();
        Path target = path(dir, build.getDirectory(), "target");
        Path classes = path(dir, build.getOutputDirectory(), "target/classes");
        Path testClasses = path(dir, build.getTestOutputDirectory(), "target/test-classes");
        Plugin compiler = lifecyclePlugin(model, COMPILER);
        Xpp3Dom main = config(compiler, "default-compile");
        Xpp3Dom test = config(compiler, "default-testCompile");
        Properties props = model.getProperties();

        JsonObject project = new JsonObject();
        project.addProperty("modelVersion", 1);
        project.addProperty("path", m.path);
        project.addProperty("name", model.getArtifactId());
        project.addProperty("projectDir", dir.getAbsolutePath());
        project.addProperty("buildFile", m.pom.getAbsolutePath());
        project.addProperty("buildDir", target.toString());
        project.addProperty("group", model.getGroupId());
        project.addProperty("version", model.getVersion());
        project.addProperty("description", model.getDescription());

        JsonObject info = new JsonObject();
        info.addProperty("rootDir", root.getAbsolutePath());
        JsonArray triggers = new JsonArray();
        for (File pom : poms) {
            triggers.add(pom.getAbsolutePath());
        }
        for (String f : List.of(".mvn/maven.config", ".mvn/extensions.xml", ".mvn/jvm.config", ".mvn/wrapper/maven-wrapper.properties")) {
            triggers.add(new File(root, f).getAbsolutePath());
        }
        triggers.add(env.m2.resolve("settings.xml").toString());
        // Parents outside the reactor that live in the folder are part of the build too.
        for (String id : m.result.getModelIds()) {
            Model raw = m.result.getRawModel(id);
            if (raw != null && raw.getPomFile() != null && !poms.contains(raw.getPomFile())) {
                triggers.add(raw.getPomFile().getAbsolutePath());
            }
        }
        info.add("resyncTriggers", triggers);
        project.add("build", info);

        JsonArray plugins = new JsonArray();
        for (Plugin p : build.getPlugins()) {
            JsonObject o = new JsonObject();
            o.addProperty("id", p.getGroupId() + ":" + p.getArtifactId());
            o.addProperty("version", p.getVersion());
            plugins.add(o);
        }
        project.add("plugins", plugins);

        // Java levels, as the compiler plugin reads them.
        String release = value(main, "release", props, "maven.compiler.release");
        String source = value(main, "source", props, "maven.compiler.source");
        String targetLevel = value(main, "target", props, "maven.compiler.target");
        if (release == null && source == null) {
            source = "1.8";
        }
        if (release == null && targetLevel == null) {
            targetLevel = source;
        }
        String testRelease = firstNonNull(value(test, "testRelease", props, "maven.compiler.testRelease"), value(test, "release", props, null), release);
        String testSource = firstNonNull(value(test, "testSource", props, "maven.compiler.testSource"), value(test, "source", props, null), source);
        String testTarget = firstNonNull(value(test, "testTarget", props, "maven.compiler.testTarget"), value(test, "target", props, null), targetLevel);
        JsonObject java = new JsonObject();
        java.addProperty("sourceCompatibility", release != null ? release : source);
        java.addProperty("targetCompatibility", release != null ? release : targetLevel);
        project.add("java", java);
        File toolchain = toolchain(model, main, options.jdks);

        if ("pom".equals(model.getPackaging())) {
            // A parent or aggregator compiles nothing: no source sets, as a Gradle root without Java.
            project.add("sourceSets", new JsonArray());
            project.add("testTasks", new JsonArray());
            project.add("springBoot", JsonNull.INSTANCE);
            project.add("maven", maven(m, root));
            JsonArray errors = new JsonArray();
            m.problems.forEach(errors::add);
            project.add("errors", errors);
            return project;
        }

        // Classpaths from one resolution of every scope.
        Resolved resolved = resolve(m, session, system, reactor, options, buildErrors);

        JsonArray sourceSets = new JsonArray();
        List<String> mainRoots = new ArrayList<>(build.getSourceDirectory() == null ? List.of() : List.of(build.getSourceDirectory()));
        List<String> testRoots = new ArrayList<>(build.getTestSourceDirectory() == null ? List.of() : List.of(build.getTestSourceDirectory()));
        addHelperSources(model, "add-source", mainRoots);
        addHelperSources(model, "add-test-source", testRoots);
        List<String> mainGenerated = inside(generated(target, main, "generatedSourcesDirectory", "generated-sources", "annotations"), dir, mainRoots);
        List<String> testGenerated = inside(generated(target, test, "generatedTestSourcesDirectory", "generated-test-sources", "test-annotations"),
            dir, testRoots);
        sourceSets.add(sourceSet("main", dir, mainRoots, mainGenerated, build.getResources(), classes, null,
            compile("compiler:compile", release, source, targetLevel, main, props, toolchain),
            resolved, false, model, session, system, reactor, options, buildErrors));
        sourceSets.add(sourceSet("test", dir, testRoots, testGenerated, build.getTestResources(), testClasses, classes,
            compile("compiler:testCompile", testRelease, testSource, testTarget, test, props, toolchain),
            resolved, true, model, session, system, reactor, options, buildErrors));
        JsonArray generators = new JsonArray();
        String generator = generator(model);
        if (generator != null) {
            generators.add(generator);
        }
        sourceSets.get(0).getAsJsonObject().add("generators", generators);
        sourceSets.get(1).getAsJsonObject().add("generators", new JsonArray());
        project.add("sourceSets", sourceSets);

        JsonArray testTasks = new JsonArray();
        JsonObject t = new JsonObject();
        t.addProperty("name", "test");
        t.addProperty("path", (":".equals(m.path) ? "" : m.path) + ":test");
        JsonArray sets = new JsonArray();
        sets.add("test");
        t.add("sourceSets", sets);
        testTasks.add(t);
        project.add("testTasks", testTasks);

        JsonObject boot = springBoot(model, mainRoots);
        project.add("springBoot", boot == null ? JsonNull.INSTANCE : boot);
        project.add("maven", maven(m, root));
        JsonArray errors = new JsonArray();
        m.problems.forEach(errors::add);
        resolved.errors.forEach(errors::add);
        project.add("errors", errors);
        return project;
    }

    private static Path path(File dir, String value, String fallback) {
        Path p = Path.of(value == null ? fallback : value);
        return (p.isAbsolute() ? p : dir.toPath().resolve(p)).normalize();
    }

    /** The source folders the build helper plugin adds ({@code add-source}, {@code add-test-source}). */
    private static void addHelperSources(Model model, String goal, List<String> out) {
        Plugin helper = plugin(model, "org.codehaus.mojo:build-helper-maven-plugin");
        if (helper == null) {
            return;
        }
        File dir = model.getProjectDirectory();
        for (PluginExecution e : helper.getExecutions()) {
            if (!e.getGoals().contains(goal)) {
                continue;
            }
            Xpp3Dom sources = child(merged(helper, e), "sources");
            if (sources == null) {
                continue;
            }
            for (Xpp3Dom s : sources.getChildren()) {
                if (s.getValue() != null && !s.getValue().isBlank()) {
                    Path p = path(dir, s.getValue().trim(), s.getValue().trim());
                    if (!out.contains(p.toString())) {
                        out.add(p.toString());
                    }
                }
            }
        }
    }

    /**
     * The generated source folders: the compiler's annotation processor output, and every
     * folder already under {@code target/generated-sources} (or {@code generated-test-sources})
     * that a code generator filled.
     */
    private static List<String> generated(Path target, Xpp3Dom config, String parameter, String folder, String annotations) {
        Set<String> out = new LinkedHashSet<>();
        String configured = text(config, parameter);
        out.add(configured != null ? path(target.getParent().toFile(), configured, configured).toString()
            : target.resolve(folder).resolve(annotations).toString());
        Path parent = target.resolve(folder);
        if (Files.isDirectory(parent)) {
            try (Stream<Path> list = Files.list(parent)) {
                list.filter(Files::isDirectory).sorted().forEach(p -> out.add(p.toString()));
            } catch (IOException e) {
                // none listed
            }
        }
        return new ArrayList<>(out);
    }

    /**
     * The generated folders, less those inside a source folder the POM declares: a generator that
     * writes its packages straight into {@code target/generated-sources}, which Build Helper adds
     * as a whole (jackson-databind's PackageVersion), would otherwise give a root per package.
     */
    private static List<String> inside(List<String> generated, File dir, List<String> declared) {
        List<Path> roots = new ArrayList<>();
        declared.forEach(d -> roots.add(path(dir, d, d)));
        List<String> out = new ArrayList<>();
        for (String g : generated) {
            Path p = Path.of(g).normalize();
            if (roots.stream().noneMatch(r -> p.startsWith(r) && !p.equals(r))) {
                out.add(g);
            }
        }
        return out;
    }

    private static JsonObject compile(String task, String release, String source, String target, Xpp3Dom config,
                                      Properties props, File toolchain) {
        JsonObject c = new JsonObject();
        c.addProperty("task", task);
        c.addProperty("type", "maven-compiler-plugin");
        c.addProperty("sourceCompatibility", source);
        c.addProperty("targetCompatibility", target);
        c.addProperty("release", release);
        c.addProperty("encoding", firstNonNull(value(config, "encoding", props, "project.build.sourceEncoding"), null));
        JsonArray args = new JsonArray();
        Xpp3Dom compilerArgs = child(config, "compilerArgs");
        if (compilerArgs != null) {
            for (Xpp3Dom a : compilerArgs.getChildren()) {
                if (a.getValue() != null) {
                    args.add(interpolate(a.getValue().trim(), props));
                }
            }
        }
        String single = text(config, "compilerArgument");
        if (single != null) {
            for (String a : single.trim().split("\\s+")) {
                args.add(a);
            }
        }
        if ("true".equals(value(config, "parameters", props, "maven.compiler.parameters")) && !contains(args, "-parameters")) {
            args.add("-parameters");
        }
        if ("true".equals(value(config, "enablePreview", props, "maven.compiler.enablePreview")) && !contains(args, "--enable-preview")) {
            args.add("--enable-preview");
        }
        c.add("compilerArgs", args);
        if (toolchain != null) {
            JsonObject tc = new JsonObject();
            JavaHomes.Jdk jdk = JavaHomes.read(toolchain);
            tc.addProperty("languageVersion", jdk == null ? null : String.valueOf(jdk.feature()));
            tc.addProperty("javaHome", toolchain.getAbsolutePath());
            c.add("toolchain", tc);
        }
        JsonObject out = new JsonObject();
        out.add("java", c);
        return out;
    }

    private static boolean contains(JsonArray a, String s) {
        for (JsonElement e : a) {
            if (s.equals(e.getAsString())) {
                return true;
            }
        }
        return false;
    }

    /**
     * The JDK a toolchain asks for ({@code maven-toolchains-plugin}'s {@code toolchains/jdk/version},
     * or the compiler's {@code jdkToolchain/version}), among those installed; null without one.
     */
    static File toolchain(Model model, Xpp3Dom compilerConfig, List<JavaHomes.Jdk> jdks) {
        String version = text(child(compilerConfig, "jdkToolchain"), "version");
        Plugin toolchains = plugin(model, "org.apache.maven.plugins:maven-toolchains-plugin");
        if (version == null && toolchains != null) {
            Xpp3Dom config = (Xpp3Dom) toolchains.getConfiguration();
            for (PluginExecution e : toolchains.getExecutions()) {
                config = config == null ? (Xpp3Dom) e.getConfiguration() : config;
                if (child(config, "toolchains") == null && e.getConfiguration() != null) {
                    config = (Xpp3Dom) e.getConfiguration();
                }
            }
            version = text(child(child(config, "toolchains"), "jdk"), "version");
        }
        if (version == null) {
            return null;
        }
        Matcher m = Pattern.compile("(\\d+)(?:\\.(\\d+))?").matcher(interpolate(version, model.getProperties()));
        if (!m.find()) {
            return null;
        }
        int feature = "1".equals(m.group(1)) && m.group(2) != null ? Integer.parseInt(m.group(2)) : Integer.parseInt(m.group(1));
        boolean atLeast = version.trim().startsWith("[") && version.contains(",");
        JavaHomes.Jdk best = null;
        for (JavaHomes.Jdk jdk : jdks) {
            if (jdk.feature() == feature) {
                return jdk.home();
            }
            if (atLeast && jdk.feature() > feature && (best == null || jdk.feature() < best.feature())) {
                best = jdk;
            }
        }
        return best == null ? null : best.home();
    }

    private static JsonObject sourceSet(String name, File dir, List<String> sourceDirs, List<String> generated, List<Resource> resources,
                                        Path output, Path mainOutput, JsonObject compile, Resolved resolved, boolean test, Model model,
                                        RepositorySystemSession session, RepositorySystem system, Reactor reactor, Options options,
                                        List<String> errors) {
        JsonObject set = new JsonObject();
        set.addProperty("name", name);
        JsonObject sources = new JsonObject();
        JsonObject java = new JsonObject();
        JsonArray dirs = new JsonArray();
        sourceDirs.forEach(d -> dirs.add(path(dir, d, d).toString()));
        java.add("srcDirs", dirs);
        java.add("includes", new JsonArray());
        java.add("excludes", new JsonArray());
        JsonArray filter = new JsonArray();
        filter.add("**/*.java");
        java.add("filter", filter);
        java.addProperty("compileTask", test ? "compiler:testCompile" : "compiler:compile");
        java.addProperty("outputDir", output.toString());
        sources.add("java", java);
        JsonObject res = new JsonObject();
        JsonArray resourceDirs = new JsonArray();
        for (Resource r : resources) {
            if (r.getDirectory() != null) {
                resourceDirs.add(path(dir, r.getDirectory(), r.getDirectory()).toString());
            }
        }
        res.add("srcDirs", resourceDirs);
        res.addProperty("outputDir", output.toString());
        sources.add("resources", res);
        set.add("sources", sources);
        set.add("javaSourceDirs", dirs);

        JsonArray roots = new JsonArray();
        for (JsonElement d : dirs) {
            roots.add(root(d.getAsString(), "source", "compiler"));
        }
        JsonArray generatedSources = new JsonArray();
        for (String g : generated) {
            if (contains(dirs, g)) {
                continue;
            }
            roots.add(root(g, "generated", "compiler"));
            JsonObject gs = new JsonObject();
            gs.addProperty("dir", g);
            gs.addProperty("kind", g.endsWith("annotations") ? "annotationProcessor" : "generator");
            gs.addProperty("task", test ? "compiler:testCompile" : "compiler:compile");
            gs.addProperty("exists", Files.isDirectory(Path.of(g)));
            generatedSources.add(gs);
        }
        for (JsonElement d : resourceDirs) {
            JsonObject r = root(d.getAsString(), "resource", null);
            r.add("languages", new JsonArray());
            roots.add(r);
        }
        set.add("roots", roots);
        set.add("generatedSources", generatedSources);
        JsonObject out = new JsonObject();
        JsonArray classes = new JsonArray();
        classes.add(output.toString());
        out.add("classesDirs", classes);
        out.addProperty("resourcesDir", output.toString());
        set.add("output", out);
        set.add("compile", compile);

        JsonArray processorPath = new JsonArray();
        for (File f : processorPath(model, test, session, system, reactor, options, errors)) {
            processorPath.add(f.getAbsolutePath());
        }
        set.add("annotationProcessorPath", processorPath);
        JsonObject configurations = new JsonObject();
        configurations.addProperty("compileClasspath", test ? "test" : "compile+provided+system");
        configurations.addProperty("runtimeClasspath", test ? "test" : "compile+runtime");
        set.add("configurations", configurations);

        Set<String> compileScopes = test ? Set.of("compile", "provided", "system", "runtime", "test") : Set.of("compile", "provided", "system");
        Set<String> runtimeScopes = test ? Set.of("compile", "provided", "system", "runtime", "test") : Set.of("compile", "runtime");
        JsonArray compileClasspath = new JsonArray();
        JsonArray runtimeClasspath = new JsonArray();
        if (test) {
            compileClasspath.add(sourceSetOutput("main", mainOutput));
            runtimeClasspath.add(sourceSetOutput("test", output));
            runtimeClasspath.add(sourceSetOutput("main", mainOutput));
        } else {
            runtimeClasspath.add(sourceSetOutput("main", output));
        }
        resolved.classpath(compileScopes, compileClasspath);
        resolved.classpath(runtimeScopes, runtimeClasspath);
        set.add("compileClasspath", compileClasspath);
        set.add("runtimeClasspath", runtimeClasspath);
        JsonArray unresolved = new JsonArray();
        resolved.unresolved.forEach(unresolved::add);
        set.add("unresolved", unresolved);
        JsonObject graphs = new JsonObject();
        graphs.add("compileClasspath", resolved.graph(compileScopes));
        graphs.add("runtimeClasspath", resolved.graph(runtimeScopes));
        set.add("dependencyGraph", graphs);
        return set;
    }

    private static JsonObject root(String dir, String kind, String compiledBy) {
        JsonObject r = new JsonObject();
        r.addProperty("dir", dir);
        r.addProperty("kind", kind);
        JsonArray languages = new JsonArray();
        languages.add("java");
        r.add("languages", languages);
        JsonArray by = new JsonArray();
        if (compiledBy != null) {
            by.add(compiledBy);
        }
        r.add("compiledBy", by);
        r.addProperty("exists", Files.isDirectory(Path.of(dir)));
        return r;
    }

    private static JsonObject sourceSetOutput(String sourceSet, Path output) {
        JsonObject o = new JsonObject();
        o.addProperty("kind", "sourceSetOutput");
        o.addProperty("sourceSet", sourceSet);
        JsonArray files = new JsonArray();
        files.add(output.toString());
        o.add("files", files);
        return o;
    }

    /** The annotation processor path the compiler plugin declares ({@code annotationProcessorPaths}), resolved. */
    private static List<File> processorPath(Model model, boolean test, RepositorySystemSession session, RepositorySystem system,
                                            Reactor reactor, Options options, List<String> errors) {
        Plugin compiler = lifecyclePlugin(model, COMPILER);
        Xpp3Dom config = config(compiler, test ? "default-testCompile" : "default-compile");
        Xpp3Dom paths = child(config, "annotationProcessorPaths");
        List<File> out = new ArrayList<>();
        if (paths == null) {
            return out;
        }
        Map<String, String> managed = new LinkedHashMap<>();
        if (model.getDependencyManagement() != null) {
            for (org.apache.maven.model.Dependency d : model.getDependencyManagement().getDependencies()) {
                managed.put(d.getGroupId() + ":" + d.getArtifactId(), d.getVersion());
            }
        }
        for (org.apache.maven.model.Dependency d : model.getDependencies()) {
            managed.putIfAbsent(d.getGroupId() + ":" + d.getArtifactId(), d.getVersion());
        }
        CollectRequest request = new CollectRequest();
        for (Xpp3Dom p : paths.getChildren()) {
            String g = interpolate(text(p, "groupId"), model.getProperties());
            String a = interpolate(text(p, "artifactId"), model.getProperties());
            String v = interpolate(text(p, "version"), model.getProperties());
            if (g == null || a == null) {
                continue;
            }
            if (v == null) {
                v = managed.get(g + ":" + a);
            }
            if (v == null) {
                continue;
            }
            String classifier = text(p, "classifier");
            String type = text(p, "type");
            request.addDependency(new Dependency(new DefaultArtifact(g, a, classifier == null ? "" : classifier,
                type == null ? "jar" : type, v), "runtime"));
        }
        request.setRepositories(repositories(model, session, system));
        try {
            DependencyResult result = system.resolveDependencies(session, new DependencyRequest(request, null));
            for (ArtifactResult r : result.getArtifactResults()) {
                if (r.getArtifact() != null && r.getArtifact().getFile() != null) {
                    out.add(r.getArtifact().getFile());
                }
            }
        } catch (DependencyResolutionException e) {
            errors.add(model.getArtifactId() + ": the annotation processor path: " + e.getMessage());
        }
        return out;
    }

    // ---- dependencies

    /** A module's resolved dependencies of every scope: Maven's classpath order, and the graph. */
    static final class Resolved {
        final List<DependencyNode> nodes = new ArrayList<>();
        DependencyNode root;
        Reactor reactor;
        Map<String, File> sources = Map.of();
        final List<String> unresolved = new ArrayList<>();
        final List<String> errors = new ArrayList<>();

        /** The entries of a classpath made of these scopes, in Maven's order. */
        void classpath(Set<String> scopes, JsonArray into) {
            Set<String> seen = new LinkedHashSet<>();
            for (DependencyNode n : nodes) {
                Dependency d = n.getDependency();
                if (d == null || !scopes.contains(d.getScope())) {
                    continue;
                }
                Artifact a = d.getArtifact();
                Reactor.Entry module = reactor.module(a);
                if (module != null) {
                    if (!seen.add("project " + module.path + " " + a.getClassifier())) {
                        continue;
                    }
                    JsonObject p = new JsonObject();
                    p.addProperty("kind", "project");
                    p.addProperty("build", ":");
                    p.addProperty("path", module.path);
                    JsonArray files = new JsonArray();
                    files.add(("tests".equals(a.getClassifier()) || "test-jar".equals(d.getArtifact().getProperty("type", "")))
                        ? module.testClasses.toString() : module.classes.toString());
                    p.add("files", files);
                    into.add(p);
                    continue;
                }
                File file = a.getFile();
                if (file == null || !seen.add(file.getPath())) {
                    continue;
                }
                JsonObject e = new JsonObject();
                e.addProperty("kind", "module");
                e.addProperty("group", a.getGroupId());
                e.addProperty("name", a.getArtifactId());
                e.addProperty("version", a.getBaseVersion());
                if (!a.getClassifier().isEmpty()) {
                    e.addProperty("classifier", a.getClassifier());
                }
                e.addProperty("scope", d.getScope());
                e.addProperty("file", file.getAbsolutePath());
                File src = sources.get(key(a));
                e.addProperty("sources", src == null ? null : src.getAbsolutePath());
                into.add(e);
            }
        }

        /**
         * The graph of the dependencies in these scopes, as a Gradle graph has it: the declared
         * dependencies as the root's edges (each with its scope), every artifact once, and the
         * version the POM asked for on an edge where dependency management chose another.
         */
        JsonObject graph(Set<String> scopes) {
            JsonObject g = new JsonObject();
            JsonArray nodesOut = new JsonArray();
            Map<String, Integer> index = new LinkedHashMap<>();
            Map<DependencyNode, Boolean> visited = new IdentityHashMap<>();
            JsonArray roots = new JsonArray();
            if (root != null) {
                for (DependencyNode child : root.getChildren()) {
                    JsonObject edge = edge(child, scopes, index, nodesOut, visited);
                    if (edge != null) {
                        edge.addProperty("declaredIn", child.getDependency().getScope());
                        roots.add(edge);
                    }
                }
            }
            g.add("roots", roots);
            g.add("nodes", nodesOut);
            return g;
        }

        private JsonObject edge(DependencyNode n, Set<String> scopes, Map<String, Integer> index, JsonArray nodesOut,
                                Map<DependencyNode, Boolean> visited) {
            Dependency d = n.getDependency();
            if (d == null || !scopes.contains(d.getScope())) {
                return null;
            }
            Artifact a = d.getArtifact();
            String key = key(a);
            Integer to = index.get(key);
            if (to == null) {
                to = nodesOut.size();
                index.put(key, to);
                JsonObject node = new JsonObject();
                Reactor.Entry module = reactor.module(a);
                if (module != null) {
                    node.addProperty("kind", "project");
                    node.addProperty("build", ":");
                    node.addProperty("path", module.path);
                    node.addProperty("name", a.getArtifactId());
                } else {
                    node.addProperty("kind", "module");
                    node.addProperty("group", a.getGroupId());
                    node.addProperty("name", a.getArtifactId() + (a.getClassifier().isEmpty() ? "" : ":" + a.getClassifier()));
                    node.addProperty("version", a.getBaseVersion());
                }
                node.add("deps", new JsonArray());
                nodesOut.add(node);
            }
            if (visited.put(n, Boolean.TRUE) == null) {
                JsonArray deps = nodesOut.get(to).getAsJsonObject().getAsJsonArray("deps");
                if (deps.isEmpty()) {
                    for (DependencyNode c : n.getChildren()) {
                        JsonObject e = edge(c, scopes, index, nodesOut, visited);
                        if (e != null) {
                            deps.add(e);
                        }
                    }
                }
            }
            JsonObject edge = new JsonObject();
            edge.addProperty("to", to);
            String premanaged = DependencyManagerUtils.getPremanagedVersion(n);
            if (premanaged != null && !premanaged.equals(a.getBaseVersion())) {
                edge.addProperty("requested", premanaged);
            }
            return edge;
        }
    }

    static String key(Artifact a) {
        return a.getGroupId() + ":" + a.getArtifactId() + ":" + a.getClassifier() + ":" + a.getBaseVersion();
    }

    private static Resolved resolve(Module m, RepositorySystemSession session, RepositorySystem system, Reactor reactor,
                                    Options options, List<String> buildErrors) {
        Model model = m.model;
        Resolved out = new Resolved();
        out.reactor = reactor;
        ArtifactTypeRegistry types = session.getArtifactTypeRegistry();
        CollectRequest request = new CollectRequest();
        request.setRootArtifact(new DefaultArtifact(model.getGroupId(), model.getArtifactId(), "", "pom", model.getVersion()));
        for (org.apache.maven.model.Dependency d : model.getDependencies()) {
            request.addDependency(dependency(d, types));
        }
        if (model.getDependencyManagement() != null) {
            for (org.apache.maven.model.Dependency d : model.getDependencyManagement().getDependencies()) {
                request.addManagedDependency(dependency(d, types));
            }
        }
        request.setRepositories(repositories(model, session, system));
        request.setRequestContext("project");
        DependencyResult result;
        try {
            result = system.resolveDependencies(session, new DependencyRequest(request, null));
        } catch (DependencyResolutionException e) {
            result = e.getResult();
            out.errors.add(firstLine(e.getMessage()));
            if (result != null) {
                for (ArtifactResult r : result.getArtifactResults()) {
                    if (!r.isResolved()) {
                        Artifact a = r.getRequest().getArtifact();
                        out.unresolved.add(a.getGroupId() + ":" + a.getArtifactId() + ":" + a.getVersion());
                    }
                }
            }
        }
        if (result == null || result.getRoot() == null) {
            return out;
        }
        out.root = result.getRoot();
        PreorderNodeListGenerator list = new PreorderNodeListGenerator();
        out.root.accept(list);
        out.nodes.addAll(list.getNodes());
        if (options.sources) {
            out.sources = sources(out.nodes, reactor, session, system, request.getRepositories());
        }
        return out;
    }

    /** The source jars of the libraries, where the repositories have them; none is fine. */
    private static Map<String, File> sources(List<DependencyNode> nodes, Reactor reactor, RepositorySystemSession session,
                                             RepositorySystem system, List<RemoteRepository> repositories) {
        List<ArtifactRequest> requests = new ArrayList<>();
        Set<String> asked = new LinkedHashSet<>();
        for (DependencyNode n : nodes) {
            if (n.getDependency() == null || reactor.module(n.getArtifact()) != null || n.getArtifact().getFile() == null
                || !n.getArtifact().getClassifier().isEmpty() || !"jar".equals(n.getArtifact().getExtension())) {
                continue;
            }
            Artifact a = n.getArtifact();
            if (asked.add(key(a))) {
                requests.add(new ArtifactRequest(new DefaultArtifact(a.getGroupId(), a.getArtifactId(), "sources", "jar", a.getVersion()),
                    n.getRepositories().isEmpty() ? repositories : n.getRepositories(), null));
            }
        }
        List<ArtifactResult> results;
        try {
            results = system.resolveArtifacts(session, requests);
        } catch (ArtifactResolutionException e) {
            results = e.getResults();
        }
        Map<String, File> out = new LinkedHashMap<>();
        for (ArtifactResult r : results) {
            if (r.isResolved() && r.getArtifact().getFile() != null) {
                Artifact a = r.getArtifact();
                out.put(a.getGroupId() + ":" + a.getArtifactId() + "::" + a.getBaseVersion(), r.getArtifact().getFile());
            }
        }
        return out;
    }

    /** The repositories a module resolves from: its POMs' (central among them), mirrored, with proxies and credentials. */
    static List<RemoteRepository> repositories(Model model, RepositorySystemSession session, RepositorySystem system) {
        List<RemoteRepository> repos = new ArrayList<>();
        for (org.apache.maven.model.Repository r : model.getRepositories()) {
            repos.add(remote(r));
        }
        if (repos.stream().noneMatch(r -> "central".equals(r.getId()))) {
            repos.add(central());
        }
        return system.newResolutionRepositories(session, repos);
    }

    static List<RemoteRepository> pluginRepositories(Model model, RepositorySystemSession session, RepositorySystem system) {
        List<RemoteRepository> repos = new ArrayList<>();
        for (org.apache.maven.model.Repository r : model.getPluginRepositories()) {
            repos.add(remote(r));
        }
        if (repos.stream().noneMatch(r -> "central".equals(r.getId()))) {
            repos.add(central());
        }
        return system.newResolutionRepositories(session, repos);
    }

    static Dependency dependency(org.apache.maven.model.Dependency d, ArtifactTypeRegistry types) {
        ArtifactType type = types.get(d.getType());
        if (type == null) {
            type = new DefaultArtifactType(d.getType());
        }
        Map<String, String> props = null;
        if (d.getSystemPath() != null && !d.getSystemPath().isEmpty()) {
            props = Collections.singletonMap(ArtifactProperties.LOCAL_PATH, d.getSystemPath());
        }
        Artifact artifact = new DefaultArtifact(d.getGroupId(), d.getArtifactId(), d.getClassifier(), null, d.getVersion(), props, type);
        List<Exclusion> exclusions = new ArrayList<>();
        for (org.apache.maven.model.Exclusion e : d.getExclusions()) {
            exclusions.add(new Exclusion(e.getGroupId(), e.getArtifactId(), "*", "*"));
        }
        return new Dependency(artifact, d.getScope(), d.getOptional() != null ? d.isOptional() : null, exclusions);
    }

    private static String firstLine(String s) {
        if (s == null) {
            return "";
        }
        int nl = s.indexOf('\n');
        return nl < 0 ? s : s.substring(0, nl);
    }

    // ---- Spring Boot and the Maven window's facts

    private static final Pattern PACKAGE = Pattern.compile("^\\s*package\\s+([\\w.]+)", Pattern.MULTILINE);
    private static final Pattern BOOT_APP = Pattern.compile("@SpringBootApplication\\b[^{]*?\\bclass\\s+(\\w+)", Pattern.DOTALL);

    /** Spring Boot's plugin: its version, the main class the build names, and the classes that could be it. */
    private static JsonObject springBoot(Model model, List<String> sourceDirs) {
        Plugin plugin = plugin(model, SPRING_BOOT);
        if (plugin == null) {
            return null;
        }
        JsonObject out = new JsonObject();
        out.addProperty("pluginVersion", plugin.getVersion());
        Xpp3Dom config = (Xpp3Dom) plugin.getConfiguration();
        String mainClass = firstNonNull(text(config, "mainClass"), model.getProperties().getProperty("start-class"));
        out.addProperty("mainClass", mainClass == null ? null : interpolate(mainClass, model.getProperties()));
        JsonArray candidates = new JsonArray();
        for (String dir : sourceDirs) {
            Path d = model.getProjectDirectory().toPath().resolve(dir);
            if (!Files.isDirectory(d)) {
                continue;
            }
            try (Stream<Path> walk = Files.walk(d)) {
                walk.filter(p -> p.toString().endsWith(".java")).limit(20000).forEach(p -> {
                    try {
                        String text = Files.readString(p);
                        Matcher m = BOOT_APP.matcher(text);
                        if (m.find()) {
                            Matcher pkg = PACKAGE.matcher(text);
                            candidates.add((pkg.find() ? pkg.group(1) + "." : "") + m.group(1));
                        }
                    } catch (IOException | RuntimeException e) {
                        // unreadable: not a candidate
                    }
                });
            } catch (IOException e) {
                // no candidates
            }
        }
        out.add("mainClassCandidates", candidates);
        JsonObject run = new JsonObject();
        run.addProperty("goal", "spring-boot:run");
        out.add("run", run);
        return out;
    }

    /** What the Maven window shows of a module: coordinates, packaging, modules, profiles and plugins. */
    private static JsonObject maven(Module m, File root) {
        Model model = m.model;
        JsonObject out = new JsonObject();
        out.addProperty("groupId", model.getGroupId());
        out.addProperty("artifactId", model.getArtifactId());
        out.addProperty("version", model.getVersion());
        out.addProperty("packaging", model.getPackaging());
        out.addProperty("name", model.getName());
        if (model.getParent() != null) {
            out.addProperty("parent", model.getParent().getGroupId() + ":" + model.getParent().getArtifactId() + ":" + model.getParent().getVersion());
        }
        JsonArray modules = new JsonArray();
        model.getModules().forEach(modules::add);
        out.add("modules", modules);
        Set<String> activeIds = new LinkedHashSet<>();
        for (String id : m.result.getModelIds()) {
            for (Profile p : m.result.getActivePomProfiles(id)) {
                activeIds.add(p.getId());
            }
        }
        for (Profile p : m.result.getActiveExternalProfiles()) {
            activeIds.add(p.getId());
        }
        JsonArray profiles = new JsonArray();
        Set<String> listed = new LinkedHashSet<>();
        for (String id : m.result.getModelIds()) {
            Model raw = m.result.getRawModel(id);
            if (raw == null || raw.getPomFile() == null) {
                continue; // the super POM's
            }
            for (Profile p : raw.getProfiles()) {
                if (!listed.add(p.getId())) {
                    continue;
                }
                JsonObject o = new JsonObject();
                o.addProperty("id", p.getId());
                o.addProperty("active", activeIds.contains(p.getId()));
                o.addProperty("activeByDefault", p.getActivation() != null && p.getActivation().isActiveByDefault());
                o.addProperty("source", raw.getPomFile().getAbsolutePath());
                profiles.add(o);
            }
        }
        out.add("profiles", profiles);
        JsonArray plugins = new JsonArray();
        Build build = model.getBuild();
        if (build != null) {
            for (Plugin p : build.getPlugins()) {
                JsonObject o = new JsonObject();
                o.addProperty("groupId", p.getGroupId());
                o.addProperty("artifactId", p.getArtifactId());
                o.addProperty("version", p.getVersion());
                o.addProperty("prefix", prefix(p.getArtifactId()));
                JsonArray goals = new JsonArray();
                Set<String> g = new LinkedHashSet<>();
                for (PluginExecution e : p.getExecutions()) {
                    g.addAll(e.getGoals());
                }
                g.forEach(goals::add);
                o.add("executionGoals", goals);
                plugins.add(o);
            }
        }
        out.add("plugins", plugins);
        JsonObject managed = new JsonObject();
        if (build != null && build.getPluginManagement() != null) {
            for (Plugin p : build.getPluginManagement().getPlugins()) {
                managed.addProperty(p.getGroupId() + ":" + p.getArtifactId(), p.getVersion());
            }
        }
        out.add("managedPlugins", managed);
        out.addProperty("wrapper", Files.isRegularFile(root.toPath().resolve(".mvn/wrapper/maven-wrapper.properties")));
        return out;
    }

    /** A plugin's goal prefix by Maven's naming convention: {@code maven-surefire-plugin} is {@code surefire}. */
    static String prefix(String artifactId) {
        if (artifactId.startsWith("maven-") && artifactId.endsWith("-plugin")) {
            return artifactId.substring(6, artifactId.length() - 7);
        }
        if (artifactId.endsWith("-maven-plugin")) {
            return artifactId.substring(0, artifactId.length() - 13);
        }
        return artifactId;
    }

    /** Phases in which plugins generate sources for the compile. */
    static final Set<String> GENERATE_PHASES = Set.of("generate-sources", "process-sources");

    /** Plugins that generate sources in {@code generate-sources} by default, without a phase in the POM. */
    static final Set<String> GENERATORS = Set.of("modello-maven-plugin", "antlr4-maven-plugin", "antlr3-maven-plugin",
        "maven-antlr-plugin", "javacc-maven-plugin", "ph-javacc-maven-plugin", "jaxb2-maven-plugin", "maven-jaxb2-plugin",
        "jaxb-maven-plugin", "cxf-codegen-plugin", "protobuf-maven-plugin", "protoc-jar-maven-plugin",
        "openapi-generator-maven-plugin", "swagger-codegen-maven-plugin", "avro-maven-plugin", "jooq-codegen-maven",
        "templating-maven-plugin", "xmlbeans-maven-plugin", "jsonschema2pojo-maven-plugin", "wsimport-maven-plugin",
        "jaxws-maven-plugin", "thrift-maven-plugin");

    /**
     * The phase to run for a module whose plugins generate sources for its compile, else null:
     * a plugin execution bound to {@code generate-sources} or {@code process-sources}, or a known
     * generator (Modello, ANTLR, JAXB, protobuf, OpenAPI...) whose goals default to them. The
     * engine runs it after the sync (see {@link Generators}). Build Helper's {@code add-source}
     * generates nothing.
     */
    static String generator(Model model) {
        if (model.getBuild() == null) {
            return null;
        }
        for (Plugin p : model.getBuild().getPlugins()) {
            if ("build-helper-maven-plugin".equals(p.getArtifactId())) {
                continue;
            }
            for (PluginExecution e : p.getExecutions()) {
                String phase = e.getPhase();
                if (phase != null ? GENERATE_PHASES.contains(phase) : GENERATORS.contains(p.getArtifactId()) && !e.getGoals().isEmpty()) {
                    return "process-sources";
                }
            }
        }
        return null;
    }

    // ---- the POM's plugin configuration

    static Plugin plugin(Model model, String key) {
        if (model.getBuild() == null) {
            return null;
        }
        for (Plugin p : model.getBuild().getPlugins()) {
            if (key.equals(p.getGroupId() + ":" + p.getArtifactId())) {
                return p;
            }
        }
        return null;
    }

    /**
     * A plugin the default lifecycle binds (the compiler), as it runs: the build's own entry, or
     * when the POM leaves it to the lifecycle, its {@code pluginManagement} entry, whose
     * configuration then applies. (Maven injects lifecycle plugins in maven-core; the model builder
     * alone leaves them out, and with them a parent's managed {@code <source>21</source>}.)
     */
    static Plugin lifecyclePlugin(Model model, String key) {
        Plugin own = plugin(model, key);
        if (own != null || model.getBuild() == null || model.getBuild().getPluginManagement() == null) {
            return own;
        }
        for (Plugin p : model.getBuild().getPluginManagement().getPlugins()) {
            if (key.equals(p.getGroupId() + ":" + p.getArtifactId())) {
                return p;
            }
        }
        return null;
    }

    /** A plugin's configuration with that of one execution merged over it. */
    static Xpp3Dom config(Plugin plugin, String execution) {
        if (plugin == null) {
            return null;
        }
        for (PluginExecution e : plugin.getExecutions()) {
            if (execution.equals(e.getId())) {
                return merged(plugin, e);
            }
        }
        return (Xpp3Dom) plugin.getConfiguration();
    }

    private static Xpp3Dom merged(Plugin plugin, PluginExecution e) {
        Xpp3Dom own = (Xpp3Dom) e.getConfiguration();
        Xpp3Dom base = (Xpp3Dom) plugin.getConfiguration();
        if (own == null) {
            return base;
        }
        return base == null ? own : Xpp3Dom.mergeXpp3Dom(new Xpp3Dom(own), new Xpp3Dom(base));
    }

    static Xpp3Dom child(Xpp3Dom dom, String name) {
        return dom == null ? null : dom.getChild(name);
    }

    static String text(Xpp3Dom dom, String name) {
        Xpp3Dom c = child(dom, name);
        return c == null || c.getValue() == null || c.getValue().isBlank() ? null : c.getValue().trim();
    }

    /** A compiler parameter: the configuration's, else the property its default reads. */
    private static String value(Xpp3Dom config, String parameter, Properties props, String property) {
        String v = text(config, parameter);
        if (v == null && property != null) {
            v = props.getProperty(property);
        }
        if (v == null) {
            return null;
        }
        v = interpolate(v.trim(), props);
        return v.isEmpty() || v.contains("${") ? null : v;
    }

    /** Fills {@code ${property}} from the model's properties, where the model left one. */
    static String interpolate(String s, Properties props) {
        if (s == null || !s.contains("${")) {
            return s;
        }
        for (int i = 0; i < 5 && s.contains("${"); i++) {
            Matcher m = PROPERTY.matcher(s);
            StringBuilder out = new StringBuilder();
            while (m.find()) {
                String v = props.getProperty(m.group(1));
                m.appendReplacement(out, Matcher.quoteReplacement(v == null ? m.group() : v));
            }
            m.appendTail(out);
            if (out.toString().equals(s)) {
                break;
            }
            s = out.toString();
        }
        return s;
    }

    private static String firstNonNull(String... values) {
        for (String v : values) {
            if (v != null) {
                return v;
            }
        }
        return null;
    }

    // ---- the reactor, as the resolver sees it

    /**
     * The modules of the reactor, so that a dependency on one resolves to the module (its POM
     * and its class folders) and not to a jar in a repository: the resolver's workspace.
     */
    static final class Reactor implements WorkspaceReader {
        record Entry(String groupId, String artifactId, String version, File pom, Path classes, Path testClasses, String path) {
        }

        private final Map<String, Entry> byGa = new LinkedHashMap<>();
        private final Map<String, File> rawPoms = new LinkedHashMap<>();
        private final WorkspaceRepository repository = new WorkspaceRepository("reactor");

        /** Notes every POM under the root whose coordinates are known from the file alone (for parents and imports). */
        void scan(File root) {
            Deque<File> queue = new ArrayDeque<>();
            queue.add(new File(root, "pom.xml"));
            Set<String> seen = new LinkedHashSet<>();
            while (!queue.isEmpty()) {
                File pom = queue.removeFirst();
                if (!pom.isFile() || !seen.add(pom.getAbsolutePath()) || seen.size() > 2000) {
                    continue;
                }
                try {
                    String text = Files.readString(pom.toPath());
                    String[] gav = rawCoordinates(text);
                    if (gav != null) {
                        rawPoms.put(gav[0] + ":" + gav[1] + ":" + gav[2], pom);
                    }
                    Matcher m = MODULE.matcher(text);
                    while (m.find()) {
                        File f = new File(pom.getParentFile(), m.group(1).trim());
                        queue.add(f.isDirectory() ? new File(f, "pom.xml") : f);
                    }
                } catch (IOException | RuntimeException e) {
                    // not part of what we can read
                }
            }
        }

        void add(Model model, File pom) {
            Build b = model.getBuild() == null ? new Build() : model.getBuild();
            File dir = pom.getParentFile();
            String path = byGa.isEmpty() ? ":" : ":" + model.getArtifactId();
            byGa.put(model.getGroupId() + ":" + model.getArtifactId(), new Entry(model.getGroupId(), model.getArtifactId(),
                model.getVersion(), pom, MavenSync.path(dir, b.getOutputDirectory(), "target/classes"),
                MavenSync.path(dir, b.getTestOutputDirectory(), "target/test-classes"), path));
            rawPoms.put(model.getGroupId() + ":" + model.getArtifactId() + ":" + model.getVersion(), pom);
        }

        /** The module an artifact is, when it is one of the reactor (same coordinates and version). */
        Entry module(Artifact a) {
            Entry e = byGa.get(a.getGroupId() + ":" + a.getArtifactId());
            return e != null && e.version.equals(a.getBaseVersion()) ? e : null;
        }

        File pom(String g, String a, String v) {
            return rawPoms.get(g + ":" + a + ":" + v);
        }

        List<Entry> entries() {
            return new ArrayList<>(byGa.values());
        }

        @Override
        public WorkspaceRepository getRepository() {
            return repository;
        }

        @Override
        public File findArtifact(Artifact artifact) {
            if ("pom".equals(artifact.getExtension())) {
                return pom(artifact.getGroupId(), artifact.getArtifactId(), artifact.getBaseVersion());
            }
            Entry e = module(artifact);
            if (e == null) {
                return null;
            }
            return "tests".equals(artifact.getClassifier()) ? e.testClasses.toFile() : e.classes.toFile();
        }

        @Override
        public List<String> findVersions(Artifact artifact) {
            Entry e = byGa.get(artifact.getGroupId() + ":" + artifact.getArtifactId());
            if (e != null) {
                return List.of(e.version);
            }
            for (String key : rawPoms.keySet()) {
                String[] p = key.split(":");
                if (p[0].equals(artifact.getGroupId()) && p[1].equals(artifact.getArtifactId())) {
                    return List.of(p[2]);
                }
            }
            return List.of();
        }
    }

    private static final Pattern PARENT_BLOCK = Pattern.compile("<parent>(.*?)</parent>", Pattern.DOTALL);
    private static final Pattern DEPENDENCIES_ETC = Pattern.compile(
        "<(parent|dependencies|dependencyManagement|build|profiles|reporting|modules|properties|distributionManagement|repositories|pluginRepositories)>.*?</\\1>",
        Pattern.DOTALL);

    /** A POM's own groupId, artifactId and version from its text, the parent's filling in; null if unknown. */
    static String[] rawCoordinates(String text) {
        String noComments = text.replaceAll("(?s)<!--.*?-->", "");
        String top = DEPENDENCIES_ETC.matcher(noComments).replaceAll("");
        Matcher parent = PARENT_BLOCK.matcher(noComments);
        String pg = null;
        String pv = null;
        if (parent.find()) {
            pg = tag(parent.group(1), "groupId");
            pv = tag(parent.group(1), "version");
        }
        String g = firstNonNull(tag(top, "groupId"), pg);
        String a = tag(top, "artifactId");
        String v = firstNonNull(tag(top, "version"), pv);
        return g == null || a == null || v == null || v.contains("${") && !v.equals("${revision}") ? null : new String[] {g, a, v};
    }

    private static String tag(String text, String name) {
        Matcher m = Pattern.compile("<" + name + ">\\s*([^<]+?)\\s*</" + name + ">").matcher(text);
        return m.find() ? m.group(1) : null;
    }

    // ---- parents and imports, through the resolver

    /** Finds parent POMs and imported BOMs: in the reactor first, then in the repositories. */
    static final class Resolver implements ModelResolver {
        private final RepositorySystem system;
        private final RepositorySystemSession session;
        private final List<RemoteRepository> repositories;
        private final Set<String> ids = new LinkedHashSet<>();
        private final Reactor reactor;

        Resolver(RepositorySystem system, RepositorySystemSession session, List<RemoteRepository> repositories, Reactor reactor) {
            this.system = system;
            this.session = session;
            this.repositories = new ArrayList<>(repositories);
            this.reactor = reactor;
            repositories.forEach(r -> ids.add(r.getId()));
        }

        @Override
        public ModelSource resolveModel(String groupId, String artifactId, String version) throws UnresolvableModelException {
            File local = reactor.pom(groupId, artifactId, version);
            if (local != null) {
                return new FileModelSource(local);
            }
            Artifact pom = new DefaultArtifact(groupId, artifactId, "", "pom", version);
            try {
                ArtifactResult r = system.resolveArtifact(session, new ArtifactRequest(pom, repositories, null));
                return new FileModelSource(r.getArtifact().getFile());
            } catch (ArtifactResolutionException e) {
                throw new UnresolvableModelException(e.getMessage(), groupId, artifactId, version, e);
            }
        }

        @Override
        public ModelSource resolveModel(org.apache.maven.model.Parent parent) throws UnresolvableModelException {
            parent.setVersion(highest(parent.getGroupId(), parent.getArtifactId(), parent.getVersion()));
            return resolveModel(parent.getGroupId(), parent.getArtifactId(), parent.getVersion());
        }

        @Override
        public ModelSource resolveModel(org.apache.maven.model.Dependency dependency) throws UnresolvableModelException {
            dependency.setVersion(highest(dependency.getGroupId(), dependency.getArtifactId(), dependency.getVersion()));
            return resolveModel(dependency.getGroupId(), dependency.getArtifactId(), dependency.getVersion());
        }

        /** A version range's highest version; a plain version as it is. */
        private String highest(String g, String a, String version) throws UnresolvableModelException {
            if (version == null || !(version.startsWith("[") || version.startsWith("("))) {
                return version;
            }
            try {
                VersionRangeResult r = system.resolveVersionRange(session,
                    new VersionRangeRequest(new DefaultArtifact(g, a, "", "pom", version), repositories, null));
                if (r.getHighestVersion() == null) {
                    throw new UnresolvableModelException("no version of " + g + ":" + a + " in " + version, g, a, version);
                }
                return r.getHighestVersion().toString();
            } catch (VersionRangeResolutionException e) {
                throw new UnresolvableModelException(e.getMessage(), g, a, version, e);
            }
        }

        @Override
        public void addRepository(org.apache.maven.model.Repository repository) throws InvalidRepositoryException {
            addRepository(repository, false);
        }

        @Override
        public void addRepository(org.apache.maven.model.Repository repository, boolean replace) throws InvalidRepositoryException {
            if (session.isIgnoreArtifactDescriptorRepositories()) {
                return;
            }
            if (!ids.add(repository.getId())) {
                if (!replace) {
                    return;
                }
                repositories.removeIf(r -> r.getId().equals(repository.getId()));
            }
            List<RemoteRepository> added = system.newResolutionRepositories(session, List.of(remote(repository)));
            repositories.addAll(added);
        }

        @Override
        public ModelResolver newCopy() {
            Resolver copy = new Resolver(system, session, repositories, reactor);
            copy.ids.addAll(ids);
            return copy;
        }
    }
}
