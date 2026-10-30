package com.snoutcode.engine.cli;

import org.apache.maven.repository.internal.MavenRepositorySystemUtils;
import org.apache.maven.settings.Mirror;
import org.apache.maven.settings.Proxy;
import org.apache.maven.settings.Server;
import org.apache.maven.settings.Settings;
import org.apache.maven.settings.building.DefaultSettingsBuilderFactory;
import org.apache.maven.settings.building.DefaultSettingsBuildingRequest;
import org.apache.maven.settings.building.SettingsBuildingException;
import org.eclipse.aether.DefaultRepositorySystemSession;
import org.eclipse.aether.RepositorySystem;
import org.eclipse.aether.repository.LocalRepository;
import org.eclipse.aether.supplier.RepositorySystemSupplier;
import org.eclipse.aether.util.repository.AuthenticationBuilder;
import org.eclipse.aether.util.repository.DefaultAuthenticationSelector;
import org.eclipse.aether.util.repository.DefaultMirrorSelector;
import org.eclipse.aether.util.repository.DefaultProxySelector;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Properties;

/**
 * Where Maven keeps its things for the user, as {@code mvn} would find them: the user's
 * {@code .m2} folder, {@code settings.xml} (mirrors, proxies, servers, profiles, the local
 * repository, offline), and the local repository; and the resolver session built on them.
 *
 * <p>The user's home is {@code -Duser.home} in {@code MAVEN_OPTS} when set, else {@code HOME},
 * else the JVM's: an app (or a test) that runs with another {@code HOME} gets another {@code .m2},
 * as {@code mvn} under that {@code HOME} would. {@code -Dmaven.repo.local}, as a system property of
 * this JVM or in {@code MAVEN_OPTS}, names the local repository, as it does for {@code mvn}.
 */
final class MavenEnv {
    /** The user's {@code .m2} folder. */
    final Path m2;
    final Settings settings;
    final Path localRepository;
    final boolean offline;

    private MavenEnv(Path m2, Settings settings, Path localRepository, boolean offline) {
        this.m2 = m2;
        this.settings = settings;
        this.localRepository = localRepository;
        this.offline = offline;
    }

    /** The environment for a build, offline when asked or when {@code settings.xml} says so. */
    static MavenEnv load(boolean offline) {
        Path m2 = userHome().resolve(".m2");
        Settings settings = readSettings(m2.resolve("settings.xml"));
        String repo = property("maven.repo.local");
        Path local = repo != null && !repo.isBlank() ? Path.of(repo)
            : settings.getLocalRepository() != null && !settings.getLocalRepository().isBlank()
                ? Path.of(settings.getLocalRepository().replace("${user.home}", userHome().toString()))
                : m2.resolve("repository");
        return new MavenEnv(m2, settings, local.toAbsolutePath().normalize(), offline || settings.isOffline());
    }

    /** The user's home for Maven: {@code -Duser.home} in {@code MAVEN_OPTS}, else {@code HOME}, else this JVM's. */
    static Path userHome() {
        String fromOpts = optsProperty("user.home");
        if (fromOpts != null && !fromOpts.isBlank()) {
            return Path.of(fromOpts);
        }
        String home = System.getenv("HOME");
        if (home != null && !home.isBlank()) {
            return Path.of(home);
        }
        return Path.of(System.getProperty("user.home"));
    }

    /** A system property of this JVM, else one {@code MAVEN_OPTS} sets with {@code -D}. */
    static String property(String name) {
        String value = System.getProperty(name);
        return value != null ? value : optsProperty(name);
    }

    /** A {@code -Dname=value} of {@code MAVEN_OPTS}, or null. */
    static String optsProperty(String name) {
        String opts = System.getenv("MAVEN_OPTS");
        if (opts == null) {
            return null;
        }
        String found = null;
        for (String token : opts.trim().split("\\s+")) {
            if (token.startsWith("-D" + name + "=")) {
                found = unquote(token.substring(name.length() + 3));
            }
        }
        return found;
    }

    private static String unquote(String s) {
        return s.length() >= 2 && (s.startsWith("\"") && s.endsWith("\"") || s.startsWith("'") && s.endsWith("'"))
            ? s.substring(1, s.length() - 1) : s;
    }

    private static Settings readSettings(Path user) {
        DefaultSettingsBuildingRequest request = new DefaultSettingsBuildingRequest();
        if (Files.isRegularFile(user)) {
            request.setUserSettingsFile(user.toFile());
        }
        String mavenHome = System.getenv("MAVEN_HOME");
        if (mavenHome != null && Files.isRegularFile(Path.of(mavenHome, "conf", "settings.xml"))) {
            request.setGlobalSettingsFile(Path.of(mavenHome, "conf", "settings.xml").toFile());
        }
        Properties system = new Properties();
        system.putAll(System.getProperties());
        system.putAll(System.getenv().entrySet().stream()
            .collect(java.util.stream.Collectors.toMap(e -> "env." + e.getKey(), Map.Entry::getValue)));
        system.setProperty("user.home", userHome().toString());
        request.setSystemProperties(system);
        try {
            return new DefaultSettingsBuilderFactory().newInstance().build(request).getEffectiveSettings();
        } catch (SettingsBuildingException e) {
            return new Settings();
        }
    }

    /** The system properties Maven sees: this JVM's, the environment as {@code env.*}, and the Maven user home. */
    Properties systemProperties() {
        Properties system = new Properties();
        system.putAll(System.getProperties());
        System.getenv().forEach((k, v) -> system.setProperty("env." + k, v));
        system.setProperty("user.home", userHome().toString());
        return system;
    }

    // ---- the resolver

    private static volatile RepositorySystem system;

    /** One repository system for the engine: it is thread safe and costly to build. */
    static RepositorySystem system() {
        RepositorySystem s = system;
        if (s == null) {
            synchronized (MavenEnv.class) {
                if (system == null) {
                    system = new RepositorySystemSupplier().get();
                }
                s = system;
            }
        }
        return s;
    }

    /**
     * A resolver session as {@code mvn} makes one: Maven's dependency selection and conflict
     * resolution, the local repository, and the mirrors, proxies, credentials and offline mode
     * of {@code settings.xml}. Premanaged versions are kept on the nodes, for the dependency
     * graphs.
     */
    DefaultRepositorySystemSession session(Properties userProperties) {
        DefaultRepositorySystemSession session = MavenRepositorySystemUtils.newSession();
        Properties system = systemProperties();
        session.setSystemProperties(system);
        session.setUserProperties(userProperties);
        Map<String, Object> config = new LinkedHashMap<>();
        system.stringPropertyNames().forEach(k -> config.put(k, system.getProperty(k)));
        userProperties.stringPropertyNames().forEach(k -> config.put(k, userProperties.getProperty(k)));
        config.put("aether.dependencyManager.verbose", "true");
        session.setConfigProperties(config);
        session.setOffline(offline);
        session.setLocalRepositoryManager(system().newLocalRepositoryManager(session, new LocalRepository(localRepository.toFile())));

        DefaultMirrorSelector mirrors = new DefaultMirrorSelector();
        for (Mirror m : settings.getMirrors()) {
            mirrors.add(m.getId(), m.getUrl(), m.getLayout(), false, m.isBlocked(), m.getMirrorOf(), m.getMirrorOfLayouts());
        }
        session.setMirrorSelector(mirrors);

        DefaultProxySelector proxies = new DefaultProxySelector();
        for (Proxy p : settings.getProxies()) {
            if (!p.isActive()) {
                continue;
            }
            AuthenticationBuilder auth = new AuthenticationBuilder();
            if (p.getUsername() != null) {
                auth.addUsername(p.getUsername()).addPassword(p.getPassword());
            }
            proxies.add(new org.eclipse.aether.repository.Proxy(p.getProtocol(), p.getHost(), p.getPort(), auth.build()),
                p.getNonProxyHosts());
        }
        session.setProxySelector(proxies);

        DefaultAuthenticationSelector auths = new DefaultAuthenticationSelector();
        for (Server s : settings.getServers()) {
            AuthenticationBuilder auth = new AuthenticationBuilder();
            if (s.getUsername() != null) {
                auth.addUsername(s.getUsername()).addPassword(s.getPassword());
            }
            if (s.getPrivateKey() != null) {
                auth.addPrivateKey(s.getPrivateKey(), s.getPassphrase());
            }
            auths.add(s.getId(), auth.build());
        }
        session.setAuthenticationSelector(auths);
        return session;
    }

    /** The profiles {@code settings.xml} activates by id. */
    List<String> activeSettingsProfiles() {
        return new ArrayList<>(settings.getActiveProfiles());
    }

    /** The settings file Maven would read for the user, when it exists (for the runs' {@code -s}). */
    File userSettingsFile() {
        Path f = m2.resolve("settings.xml");
        return Files.isRegularFile(f) ? f.toFile() : null;
    }
}
