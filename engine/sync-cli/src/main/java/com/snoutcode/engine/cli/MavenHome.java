package com.snoutcode.engine.cli;

import java.io.BufferedInputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.DigestInputStream;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Properties;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.zip.GZIPInputStream;

/**
 * The Maven that runs a build's goals, so the user needs no Maven of their own: the build's
 * wrapper ({@code mvnw}) when it has one, which fetches the Maven its properties name; else a
 * pinned Apache Maven, downloaded once from Maven Central into the cache directory
 * ({@code maven/apache-maven-<version>}) and checked against its SHA-512, the way the app
 * downloads JDKs.
 */
final class MavenHome {
    /** The Maven the engine downloads, and the version of the libraries it imports builds with. */
    static final String VERSION = "3.9.16";
    static final String SHA512 =
        "831a8591fe20c8243b1dbe7d71e3244f31d1665b0804b2e825e38cbbe5ce0cafb8338851f90780735568773e0a6cd07bbec107cda0b896b008b861075358b6f6";
    static final String URL = "https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/" + VERSION
        + "/apache-maven-" + VERSION + "-bin.tar.gz";

    private static volatile Path cacheDir;

    private MavenHome() {
    }

    /** Where the downloaded Maven goes: the engine's cache directory. */
    static void cacheDir(Path dir) {
        cacheDir = dir;
    }

    static Path cacheDir() {
        Path dir = cacheDir;
        return dir != null ? dir : Lsp.defaultCacheDir();
    }

    /** The build's wrapper script, when it has one. */
    static File wrapper(File root) {
        boolean windows = System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT).contains("win");
        File script = new File(root, windows ? "mvnw.cmd" : "mvnw");
        return script.isFile() && Files.isRegularFile(root.toPath().resolve(".mvn/wrapper/maven-wrapper.properties")) ? script : null;
    }

    private static final Pattern DIST_VERSION = Pattern.compile("apache-maven-([^/]+?)-bin\\.(?:zip|tar\\.gz)");

    /** The Maven version a build runs: its wrapper's, else the one the engine downloads. */
    static String wrapperVersion(File root) {
        Path props = root.toPath().resolve(".mvn/wrapper/maven-wrapper.properties");
        if (Files.isRegularFile(props)) {
            Properties p = new Properties();
            try (InputStream in = Files.newInputStream(props)) {
                p.load(in);
                Matcher m = DIST_VERSION.matcher(String.valueOf(p.getProperty("distributionUrl")));
                if (m.find()) {
                    return m.group(1);
                }
            } catch (IOException e) {
                // fall through
            }
        }
        return VERSION;
    }

    /**
     * The command that starts Maven for a build: its wrapper, else the downloaded Maven's
     * {@code bin/mvn}, downloading it first when it is not there yet.
     */
    static List<String> command(File root, PrintStream progress) throws IOException {
        List<String> out = new ArrayList<>();
        File wrapper = wrapper(root);
        if (wrapper != null) {
            if (!wrapper.canExecute() && !System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT).contains("win")) {
                out.add("/bin/sh");
            }
            out.add(wrapper.getAbsolutePath());
            return out;
        }
        out.add(installed(progress).resolve("bin").resolve(windows() ? "mvn.cmd" : "mvn").toString());
        return out;
    }

    private static boolean windows() {
        return System.getProperty("os.name", "").toLowerCase(java.util.Locale.ROOT).contains("win");
    }

    /** The downloaded Maven's home, downloaded when missing. */
    static synchronized Path installed(PrintStream progress) throws IOException {
        Path parent = cacheDir().resolve("maven");
        Path home = parent.resolve("apache-maven-" + VERSION);
        if (Files.isRegularFile(home.resolve("bin/mvn")) && Files.isRegularFile(home.resolve(".snout-complete"))) {
            return home;
        }
        Files.createDirectories(parent);
        if (progress != null) {
            progress.println("Downloading Apache Maven " + VERSION + " from Maven Central");
        }
        Path archive = Files.createTempFile(parent, "apache-maven-", ".tar.gz.part");
        try {
            download(URI.create(URL), archive);
            String sum = sha512(archive);
            if (!SHA512.equals(sum)) {
                throw new IOException("the Maven download's SHA-512 is " + sum + ", not " + SHA512);
            }
            Path staging = Files.createTempDirectory(parent, "unpack-");
            try {
                untar(archive, staging);
                Path unpacked = staging.resolve("apache-maven-" + VERSION);
                if (!Files.isRegularFile(unpacked.resolve("bin/mvn"))) {
                    throw new IOException("the Maven download holds no bin/mvn");
                }
                Files.writeString(unpacked.resolve(".snout-complete"), URL + "\n");
                deleteTree(home);
                Files.move(unpacked, home, StandardCopyOption.ATOMIC_MOVE);
            } finally {
                deleteTree(staging);
            }
        } finally {
            Files.deleteIfExists(archive);
        }
        if (progress != null) {
            progress.println("Apache Maven " + VERSION + " is in " + home);
        }
        return home;
    }

    private static void download(URI uri, Path to) throws IOException {
        HttpClient client = HttpClient.newBuilder().followRedirects(HttpClient.Redirect.NORMAL)
            .connectTimeout(Duration.ofSeconds(30)).build();
        HttpRequest request = HttpRequest.newBuilder(uri).timeout(Duration.ofMinutes(10)).GET().build();
        try {
            HttpResponse<Path> response = client.send(request, HttpResponse.BodyHandlers.ofFile(to));
            if (response.statusCode() != 200) {
                throw new IOException("downloading " + uri + " answered HTTP " + response.statusCode());
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("interrupted while downloading " + uri, e);
        }
    }

    static String sha512(Path file) throws IOException {
        try (InputStream in = new DigestInputStream(new BufferedInputStream(Files.newInputStream(file)),
            MessageDigest.getInstance("SHA-512"))) {
            in.transferTo(OutputStream.nullOutputStream());
            return HexFormat.of().formatHex(((DigestInputStream) in).getMessageDigest().digest());
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }

    /** Unpacks a {@code .tar.gz} (regular files, folders, the executable bit). */
    static void untar(Path archive, Path into) throws IOException {
        try (InputStream in = new BufferedInputStream(new GZIPInputStream(Files.newInputStream(archive)))) {
            byte[] header = new byte[512];
            String longName = null;
            while (in.readNBytes(header, 0, 512) == 512) {
                if (header[0] == 0) {
                    break;
                }
                String name = longName != null ? longName : field(header, 0, 100);
                longName = null;
                String prefix = field(header, 345, 155);
                if (!prefix.isEmpty()) {
                    name = prefix + "/" + name;
                }
                long size = Long.parseLong(field(header, 124, 12).trim().isEmpty() ? "0" : field(header, 124, 12).trim(), 8);
                int mode = Integer.parseInt(field(header, 100, 8).trim().isEmpty() ? "644" : field(header, 100, 8).trim(), 8);
                char type = (char) header[156];
                long padded = (size + 511) / 512 * 512;
                if (type == 'L') {
                    byte[] data = in.readNBytes((int) padded);
                    longName = new String(data, 0, (int) size, StandardCharsets.UTF_8).replace("\0", "");
                    continue;
                }
                Path target = into.resolve(name).normalize();
                if (!target.startsWith(into)) {
                    throw new IOException("the archive leaves its folder: " + name);
                }
                if (type == '5') {
                    Files.createDirectories(target);
                } else if (type == '0' || type == 0) {
                    Files.createDirectories(target.getParent());
                    try (OutputStream out = Files.newOutputStream(target)) {
                        long left = size;
                        byte[] buf = new byte[8192];
                        while (left > 0) {
                            int n = in.read(buf, 0, (int) Math.min(buf.length, left));
                            if (n < 0) {
                                throw new IOException("the archive ends early");
                            }
                            out.write(buf, 0, n);
                            left -= n;
                        }
                    }
                    if ((mode & 0100) != 0) {
                        target.toFile().setExecutable(true, false);
                    }
                    in.skipNBytes(padded - size);
                    continue;
                }
                in.skipNBytes(padded);
            }
        }
    }

    private static String field(byte[] header, int at, int length) {
        int end = at;
        while (end < at + length && header[end] != 0) {
            end++;
        }
        return new String(header, at, end - at, StandardCharsets.UTF_8);
    }

    private static void deleteTree(Path dir) throws IOException {
        if (!Files.exists(dir)) {
            return;
        }
        try (var walk = Files.walk(dir)) {
            for (Path p : walk.sorted(java.util.Comparator.reverseOrder()).toList()) {
                Files.deleteIfExists(p);
            }
        }
    }
}
