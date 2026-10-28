//! Writing a new Gradle project: a plain Java one, made here, and a Spring Boot one, from
//! start.spring.io or (without the network) from a template of our own. No drawing; every
//! function here is blocking and runs off the main thread.

use std::path::Path;

use anyhow::{Context as _, bail};
use snout_host::{Cmd, Host};

/// The Gradle a Java project's wrapper runs: the one the engine builds with, so it is
/// usually on this Mac already and a new project syncs without downloading Gradle.
pub const GRADLE_VERSION: &str = "8.10.2";
/// The Gradle of a Spring Boot project made here: Spring Boot 4 needs 8.14 or later, and
/// this is the one start.spring.io gives its projects.
pub const BOOT_GRADLE_VERSION: &str = "9.8.2";
const WRAPPER_JAR: &[u8] = include_bytes!("../../../../engine/gradle/wrapper/gradle-wrapper.jar");
const GRADLEW: &str = include_str!("../../../../engine/gradlew");
const GRADLEW_BAT: &str = include_str!("../../../../engine/gradlew.bat");

/// The build script's language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dsl {
    Kotlin,
    Groovy,
}

impl Dsl {
    pub fn build_file(self) -> &'static str {
        match self {
            Dsl::Kotlin => "build.gradle.kts",
            Dsl::Groovy => "build.gradle",
        }
    }

    pub fn settings_file(self) -> &'static str {
        match self {
            Dsl::Kotlin => "settings.gradle.kts",
            Dsl::Groovy => "settings.gradle",
        }
    }
}

/// What every new project is asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Basics {
    /// The folder's name and the Gradle root project's.
    pub name: String,
    pub group: String,
    pub artifact: String,
    pub package: String,
    /// The Java level, as Java names it (8, 17, 21).
    pub java: u32,
    pub dsl: Dsl,
}

/// A Spring Boot project's choices besides the basics.
#[derive(Clone, Debug, PartialEq)]
pub struct Boot {
    pub version: String,
    /// The Initializr's ids of the dependencies (`web`, `data-jpa`).
    pub dependencies: Vec<String>,
}

/// Whether `dir` on `host` is there and holds anything (or cannot be read).
pub fn occupied(host: &dyn Host, dir: &Path) -> bool {
    host.exists(dir) && host.read_dir(dir).map(|d| !d.is_empty()).unwrap_or(true)
}

/// The words for a project folder that [`occupied`] says is taken.
pub fn occupied_problem(dir: &Path) -> String {
    format!("{} already exists and is not empty.", dir.display())
}

/// Why `b` cannot make a project in `location`, if it cannot: the first problem, in words.
/// `occupied` says whether the project's folder there is taken, when that is known.
pub fn problem(b: &Basics, location: &Path, occupied: Option<bool>) -> Option<String> {
    let name = b.name.trim();
    if name.is_empty() {
        return Some("Give the project a name.".into());
    }
    if name.starts_with('.') || name.contains(['/', ':', '\\']) {
        return Some("The name cannot start with a dot or hold / : or \\.".into());
    }
    if location.as_os_str().is_empty() || !location.is_absolute() {
        return Some("Choose where the project goes.".into());
    }
    if occupied == Some(true) {
        return Some(occupied_problem(&location.join(name)));
    }
    if !is_dotted(&b.group, false) {
        return Some("The group is a dotted name, such as com.example.".into());
    }
    if b.artifact.trim().is_empty() || !b.artifact.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        return Some("The artifact holds letters, digits, dots, dashes and underscores.".into());
    }
    if !is_dotted(&b.package, true) {
        return Some("The package is a dotted Java name, such as com.example.demo.".into());
    }
    None
}

/// `com.example`: dotted parts, each a Java identifier (and not a keyword, when `java`).
fn is_dotted(s: &str, java: bool) -> bool {
    const KEYWORDS: [&str; 12] =
        ["class", "int", "new", "public", "static", "void", "package", "import", "default", "case", "for", "if"];
    !s.is_empty()
        && s.split('.').all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !(java && KEYWORDS.contains(&part))
        })
}

/// The package for a group and an artifact: `com.example` and `my-app` give
/// `com.example.myapp`, as the Initializr makes it.
pub fn package_for(group: &str, artifact: &str) -> String {
    let artifact: String = artifact.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    let artifact = artifact.to_lowercase();
    match (group.trim(), artifact.as_str()) {
        ("", a) => a.to_owned(),
        (g, "") => g.to_owned(),
        (g, a) => format!("{g}.{a}"),
    }
}

/// `my-shop` as a class name's start: `MyShop`.
pub fn class_stem(name: &str) -> String {
    let stem: String = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        })
        .collect();
    if stem.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) { stem } else { format!("App{stem}") }
}

fn write(host: &dyn Host, dir: &Path, rel: &str, text: &str) -> anyhow::Result<()> {
    write_bytes(host, dir, rel, text.as_bytes(), None)
}

fn write_bytes(host: &dyn Host, dir: &Path, rel: &str, bytes: &[u8], mode: Option<u32>) -> anyhow::Result<()> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        host.create_dir_all(parent)?;
    }
    host.write_mode(&path, bytes, mode).with_context(|| format!("write {}", path.display()))
}

/// The Gradle wrapper, so the project builds with `./gradlew` and a known Gradle.
fn write_wrapper(host: &dyn Host, dir: &Path, version: &str) -> anyhow::Result<()> {
    write(
        host,
        dir,
        "gradle/wrapper/gradle-wrapper.properties",
        &format!(
            "distributionBase=GRADLE_USER_HOME\ndistributionPath=wrapper/dists\n\
             distributionUrl=https\\://services.gradle.org/distributions/gradle-{version}-bin.zip\n\
             networkTimeout=10000\nvalidateDistributionUrl=true\nzipStoreBase=GRADLE_USER_HOME\nzipStorePath=wrapper/dists\n"
        ),
    )?;
    write_bytes(host, dir, "gradle/wrapper/gradle-wrapper.jar", WRAPPER_JAR, None)?;
    write_bytes(host, dir, "gradlew", GRADLEW.as_bytes(), Some(0o755))?;
    write(host, dir, "gradlew.bat", GRADLEW_BAT)?;
    Ok(())
}

const GITIGNORE: &str = ".gradle/\nbuild/\n!gradle/wrapper/gradle-wrapper.jar\nout/\nbin/\n*.class\n.DS_Store\n";

/// A plain Java application built by Gradle, in `dir` (made if missing): the wrapper, the
/// build and settings scripts, a main class and a JUnit test for it.
pub fn write_java(host: &dyn Host, dir: &Path, b: &Basics) -> anyhow::Result<()> {
    host.create_dir_all(dir)?;
    let main = format!("{}.Main", b.package);
    let (settings, build) = match b.dsl {
        Dsl::Kotlin => (
            format!(
                "plugins {{\n    // Lets Gradle download the Java the toolchain asks for.\n    \
                 id(\"org.gradle.toolchains.foojay-resolver-convention\") version \"0.8.0\"\n}}\n\n\
                 rootProject.name = \"{}\"\n",
                b.name
            ),
            format!(
                "plugins {{\n    application\n}}\n\ngroup = \"{group}\"\nversion = \"1.0-SNAPSHOT\"\n\n\
                 repositories {{\n    mavenCentral()\n}}\n\n\
                 dependencies {{\n    testImplementation(platform(\"org.junit:junit-bom:5.11.3\"))\n    \
                 testImplementation(\"org.junit.jupiter:junit-jupiter\")\n    \
                 testRuntimeOnly(\"org.junit.platform:junit-platform-launcher\")\n}}\n\n\
                 java {{\n    toolchain {{\n        languageVersion = JavaLanguageVersion.of({java})\n    }}\n}}\n\n\
                 application {{\n    mainClass = \"{main}\"\n}}\n\n\
                 tasks.test {{\n    useJUnitPlatform()\n}}\n",
                group = b.group,
                java = b.java,
            ),
        ),
        Dsl::Groovy => (
            format!(
                "plugins {{\n    // Lets Gradle download the Java the toolchain asks for.\n    \
                 id 'org.gradle.toolchains.foojay-resolver-convention' version '0.8.0'\n}}\n\n\
                 rootProject.name = '{}'\n",
                b.name
            ),
            format!(
                "plugins {{\n    id 'application'\n}}\n\ngroup = '{group}'\nversion = '1.0-SNAPSHOT'\n\n\
                 repositories {{\n    mavenCentral()\n}}\n\n\
                 dependencies {{\n    testImplementation platform('org.junit:junit-bom:5.11.3')\n    \
                 testImplementation 'org.junit.jupiter:junit-jupiter'\n    \
                 testRuntimeOnly 'org.junit.platform:junit-platform-launcher'\n}}\n\n\
                 java {{\n    toolchain {{\n        languageVersion = JavaLanguageVersion.of({java})\n    }}\n}}\n\n\
                 application {{\n    mainClass = '{main}'\n}}\n\n\
                 tasks.named('test') {{\n    useJUnitPlatform()\n}}\n",
                group = b.group,
                java = b.java,
            ),
        ),
    };
    write(host, dir, b.dsl.settings_file(), &settings)?;
    write(host, dir, b.dsl.build_file(), &build)?;
    let pkg_dir = b.package.replace('.', "/");
    let pkg = &b.package;
    write(
        host,
        dir,
        &format!("src/main/java/{pkg_dir}/Main.java"),
        &format!(
            "package {pkg};\n\npublic class Main {{\n    public static void main(String[] args) {{\n        \
             System.out.println(greeting(\"{name}\"));\n    }}\n\n    \
             static String greeting(String name) {{\n        return \"Hello from \" + name + \"!\";\n    }}\n}}\n",
            name = b.name
        ),
    )?;
    write(
        host,
        dir,
        &format!("src/test/java/{pkg_dir}/MainTest.java"),
        &format!(
            "package {pkg};\n\nimport org.junit.jupiter.api.Test;\n\n\
             import static org.junit.jupiter.api.Assertions.assertEquals;\n\n\
             class MainTest {{\n    @Test\n    void greets() {{\n        \
             assertEquals(\"Hello from you!\", Main.greeting(\"you\"));\n    }}\n}}\n"
        ),
    )?;
    host.create_dir_all(&dir.join("src/main/resources"))?;
    write(host, dir, ".gitignore", GITIGNORE)?;
    write_wrapper(host, dir, GRADLE_VERSION)
}

/// A dependency the offline Spring Boot template knows: its Initializr id and what it adds.
pub struct KnownDependency {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    /// Gradle configuration and coordinates, each line one dependency.
    pub lines: &'static [(&'static str, &'static str)],
}

/// The Spring Boot version the offline template uses when start.spring.io never answered.
pub const OFFLINE_BOOT: &str = "4.1.2";
/// The dependency management plugin start.spring.io pairs with it.
const DEPENDENCY_MANAGEMENT: &str = "1.1.7";

/// What the offline template can add, in the Initializr's groups.
pub const KNOWN: &[KnownDependency] = &[
    KnownDependency {
        id: "devtools",
        name: "Spring Boot DevTools",
        group: "Developer Tools",
        description: "Fast restarts and live reload while developing.",
        lines: &[("developmentOnly", "org.springframework.boot:spring-boot-devtools")],
    },
    KnownDependency {
        id: "lombok",
        name: "Lombok",
        group: "Developer Tools",
        description: "Annotations that write the boilerplate for you.",
        lines: &[
            ("compileOnly", "org.projectlombok:lombok"),
            ("annotationProcessor", "org.projectlombok:lombok"),
            ("testCompileOnly", "org.projectlombok:lombok"),
            ("testAnnotationProcessor", "org.projectlombok:lombok"),
        ],
    },
    KnownDependency {
        id: "web",
        name: "Spring Web",
        group: "Web",
        description: "Web applications and REST services with Spring MVC and Tomcat.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-webmvc"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-webmvc-test"),
        ],
    },
    KnownDependency {
        id: "webflux",
        name: "Spring Reactive Web",
        group: "Web",
        description: "Reactive web applications with Spring WebFlux and Netty.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-webflux"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-webflux-test"),
        ],
    },
    KnownDependency {
        id: "thymeleaf",
        name: "Thymeleaf",
        group: "Template Engines",
        description: "Server side HTML templates.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-thymeleaf"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-thymeleaf-test"),
        ],
    },
    KnownDependency {
        id: "security",
        name: "Spring Security",
        group: "Security",
        description: "Authentication and access control.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-security"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-security-test"),
        ],
    },
    KnownDependency {
        id: "data-jpa",
        name: "Spring Data JPA",
        group: "SQL",
        description: "SQL databases through the Java Persistence API and Hibernate.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-data-jpa"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-data-jpa-test"),
        ],
    },
    KnownDependency {
        id: "jdbc",
        name: "JDBC API",
        group: "SQL",
        description: "SQL databases through JDBC and a connection pool.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-jdbc"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-jdbc-test"),
        ],
    },
    KnownDependency {
        id: "h2",
        name: "H2 Database",
        group: "SQL",
        description: "A fast in memory database.",
        lines: &[("runtimeOnly", "com.h2database:h2")],
    },
    KnownDependency {
        id: "postgresql",
        name: "PostgreSQL Driver",
        group: "SQL",
        description: "The JDBC driver for PostgreSQL.",
        lines: &[("runtimeOnly", "org.postgresql:postgresql")],
    },
    KnownDependency {
        id: "validation",
        name: "Validation",
        group: "I/O",
        description: "Bean Validation with Hibernate Validator.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-validation"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-validation-test"),
        ],
    },
    KnownDependency {
        id: "actuator",
        name: "Spring Boot Actuator",
        group: "Ops",
        description: "Health, metrics and other endpoints to watch the application.",
        lines: &[
            ("implementation", "org.springframework.boot:spring-boot-starter-actuator"),
            ("testImplementation", "org.springframework.boot:spring-boot-starter-actuator-test"),
        ],
    },
];

/// A Spring Boot application made here, for when start.spring.io cannot be reached: the
/// template start.spring.io itself makes, with the dependencies of [`KNOWN`] among those
/// chosen (the others are left out). Gradle downloads Spring Boot once it has the network.
pub fn write_boot_offline(host: &dyn Host, dir: &Path, b: &Basics, boot: &Boot) -> anyhow::Result<()> {
    host.create_dir_all(dir)?;
    let kotlin = b.dsl == Dsl::Kotlin;
    let dep = |conf: &str, coords: &str| {
        if kotlin { format!("    {conf}(\"{coords}\")\n") } else { format!("    {conf} '{coords}'\n") }
    };
    let chosen: Vec<&KnownDependency> = KNOWN.iter().filter(|k| boot.dependencies.iter().any(|d| d == k.id)).collect();
    // As start.spring.io writes them for Spring Boot 4: each starter with its test starter,
    // the plain starters when none is chosen, grouped by configuration.
    let mut lines: Vec<(&str, &str)> = chosen.iter().flat_map(|k| k.lines.iter().copied()).collect();
    let web = chosen.iter().any(|k| k.id == "web" || k.id == "webflux");
    if !lines.iter().any(|(c, a)| *c == "implementation" && a.starts_with("org.springframework.boot:")) {
        lines.push(("implementation", "org.springframework.boot:spring-boot-starter"));
        lines.push(("testImplementation", "org.springframework.boot:spring-boot-starter-test"));
    }
    lines.push(("testRuntimeOnly", "org.junit.platform:junit-platform-launcher"));
    const ORDER: [&str; 8] = [
        "implementation",
        "compileOnly",
        "developmentOnly",
        "runtimeOnly",
        "annotationProcessor",
        "testImplementation",
        "testCompileOnly",
        "testRuntimeOnly",
    ];
    lines.sort_by_key(|(c, a)| (ORDER.iter().position(|o| o == c).unwrap_or(ORDER.len()), *a));
    lines.dedup();
    let deps: String = lines.iter().map(|(c, a)| dep(c, a)).collect();
    let (settings, build) = if kotlin {
        (
            format!("rootProject.name = \"{}\"\n", b.name),
            format!(
                "plugins {{\n    java\n    id(\"org.springframework.boot\") version \"{v}\"\n    \
                 id(\"io.spring.dependency-management\") version \"{DEPENDENCY_MANAGEMENT}\"\n}}\n\n\
                 group = \"{group}\"\nversion = \"0.0.1-SNAPSHOT\"\n\n\
                 java {{\n    toolchain {{\n        languageVersion = JavaLanguageVersion.of({java})\n    }}\n}}\n\n\
                 repositories {{\n    mavenCentral()\n}}\n\ndependencies {{\n{deps}}}\n\n\
                 tasks.withType<Test> {{\n    useJUnitPlatform()\n}}\n",
                v = boot.version,
                group = b.group,
                java = b.java,
            ),
        )
    } else {
        (
            format!("rootProject.name = '{}'\n", b.name),
            format!(
                "plugins {{\n    id 'java'\n    id 'org.springframework.boot' version '{v}'\n    \
                 id 'io.spring.dependency-management' version '{DEPENDENCY_MANAGEMENT}'\n}}\n\n\
                 group = '{group}'\nversion = '0.0.1-SNAPSHOT'\n\n\
                 java {{\n    toolchain {{\n        languageVersion = JavaLanguageVersion.of({java})\n    }}\n}}\n\n\
                 repositories {{\n    mavenCentral()\n}}\n\ndependencies {{\n{deps}}}\n\n\
                 tasks.named('test') {{\n    useJUnitPlatform()\n}}\n",
                v = boot.version,
                group = b.group,
                java = b.java,
            ),
        )
    };
    write(host, dir, b.dsl.settings_file(), &settings)?;
    write(host, dir, b.dsl.build_file(), &build)?;
    let pkg_dir = b.package.replace('.', "/");
    let pkg = &b.package;
    let app = format!("{}Application", class_stem(&b.name));
    write(
        host,
        dir,
        &format!("src/main/java/{pkg_dir}/{app}.java"),
        &format!(
            "package {pkg};\n\nimport org.springframework.boot.SpringApplication;\n\
             import org.springframework.boot.autoconfigure.SpringBootApplication;\n\n\
             @SpringBootApplication\npublic class {app} {{\n\n    public static void main(String[] args) {{\n        \
             SpringApplication.run({app}.class, args);\n    }}\n\n}}\n"
        ),
    )?;
    write(
        host,
        dir,
        &format!("src/test/java/{pkg_dir}/{app}Tests.java"),
        &format!(
            "package {pkg};\n\nimport org.junit.jupiter.api.Test;\n\
             import org.springframework.boot.test.context.SpringBootTest;\n\n\
             @SpringBootTest\nclass {app}Tests {{\n\n    @Test\n    void contextLoads() {{\n    }}\n\n}}\n"
        ),
    )?;
    write(host, dir, "src/main/resources/application.properties", &format!("spring.application.name={}\n", b.name))?;
    if web {
        host.create_dir_all(&dir.join("src/main/resources/static"))?;
        host.create_dir_all(&dir.join("src/main/resources/templates"))?;
    }
    write(host, dir, ".gitignore", GITIGNORE)?;
    write_wrapper(host, dir, BOOT_GRADLE_VERSION)
}

/// The address of start.spring.io's zip for `b` and `boot`.
pub fn starter_url(base: &str, b: &Basics, boot: &Boot) -> String {
    let kind = match b.dsl {
        Dsl::Kotlin => "gradle-project-kotlin",
        Dsl::Groovy => "gradle-project",
    };
    let q = |s: &str| encode(s);
    let mut url = format!(
        "{base}/starter.zip?type={kind}&language=java&packaging=jar&bootVersion={}&baseDir={}&groupId={}\
         &artifactId={}&name={}&packageName={}&javaVersion={}",
        q(&boot.version),
        q(&b.name),
        q(&b.group),
        q(&b.artifact),
        q(&b.name),
        q(&b.package),
        b.java
    );
    if !boot.dependencies.is_empty() {
        url += &format!("&dependencies={}", q(&boot.dependencies.join(",")));
    }
    url
}

/// Percent-encode what is not plainly safe in a query.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Download the project for `b` and `boot` from start.spring.io into `dir` (which must not
/// exist or be empty). Fails with the reason, in words.
pub fn download_boot(host: &dyn Host, base: &str, dir: &Path, b: &Basics, boot: &Boot) -> anyhow::Result<()> {
    let parent = dir.parent().context("no folder above the project")?;
    host.create_dir_all(parent)?;
    let stage = parent.join(format!(".{}.new-project", b.name));
    let _ = host.remove_dir_all(&stage);
    host.create_dir_all(&stage)?;
    let result = (|| {
        let zip = stage.join("starter.zip");
        let url = starter_url(base, b, boot);
        let curl =
            Cmd::new("curl").args(["-fsSL", "--max-time", "60", "--connect-timeout", "10", "-o"]).arg(&zip).arg(&url);
        let out = host.run(&curl, None).context("run curl")?;
        if !out.status.success() {
            let why = String::from_utf8_lossy(&out.stderr).trim().trim_start_matches("curl: ").to_owned();
            bail!("start.spring.io could not make the project ({why})");
        }
        let unpacked = stage.join("unpacked");
        let unzip = if host.info().is_mac() {
            Cmd::new("/usr/bin/ditto").arg("-x").arg("-k").arg(&zip).arg(&unpacked)
        } else {
            Cmd::new("unzip").arg("-q").arg(&zip).arg("-d").arg(&unpacked)
        };
        let ok = host.run(&unzip, None)?.status.success();
        if !ok {
            bail!("the project from start.spring.io could not be unpacked");
        }
        // The zip holds one folder (baseDir); its contents are the project.
        let top = host.read_dir(&unpacked)?;
        let from = match top.as_slice() {
            [one] if one.is_dir => one.path.clone(),
            _ => unpacked.clone(),
        };
        if host.exists(dir) {
            host.remove_dir(dir).context("the project folder is not empty")?;
        }
        host.rename(&from, dir).context("move the project into place")?;
        Ok(())
    })();
    let _ = host.remove_dir_all(&stage);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn basics(dsl: Dsl) -> Basics {
        Basics {
            name: "my-shop".into(),
            group: "com.example".into(),
            artifact: "my-shop".into(),
            package: "com.example.myshop".into(),
            java: 21,
            dsl,
        }
    }

    #[test]
    fn names_and_problems() {
        assert_eq!(package_for("com.example", "my-shop"), "com.example.myshop");
        assert_eq!(class_stem("my-shop"), "MyShop");
        assert_eq!(class_stem("2fast"), "App2fast");
        let dir = std::env::temp_dir().join(format!("snout-new-names-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("taken")).unwrap();
        std::fs::write(dir.join("taken/x"), "").unwrap();
        let mut b = basics(Dsl::Kotlin);
        let host = snout_host::local();
        let occupied = |b: &Basics| Some(occupied(&*host, &dir.join(&b.name)));
        assert_eq!(problem(&b, &dir, occupied(&b)), None);
        b.name = "taken".into();
        assert!(problem(&b, &dir, occupied(&b)).unwrap().contains("not empty"));
        b.name = "ok".into();
        b.package = "com.example.class".into();
        assert!(problem(&b, &dir, None).unwrap().contains("package"));
        b.package = "com.example".into();
        b.group = "com..example".into();
        assert!(problem(&b, &dir, None).unwrap().contains("group"));
        assert!(problem(&basics(Dsl::Kotlin), Path::new("relative"), None).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn writes_a_java_project_in_either_dsl() {
        for dsl in [Dsl::Kotlin, Dsl::Groovy] {
            let dir = std::env::temp_dir().join(format!("snout-new-java-{dsl:?}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            write_java(&*snout_host::local(), &dir, &basics(dsl)).unwrap();
            let build = std::fs::read_to_string(dir.join(dsl.build_file())).unwrap();
            assert!(build.contains("JavaLanguageVersion.of(21)"), "{build}");
            assert!(build.contains("com.example.myshop.Main"), "{build}");
            assert!(std::fs::read_to_string(dir.join(dsl.settings_file())).unwrap().contains("my-shop"));
            assert!(dir.join("src/main/java/com/example/myshop/Main.java").is_file());
            assert!(dir.join("src/test/java/com/example/myshop/MainTest.java").is_file());
            assert!(dir.join("gradle/wrapper/gradle-wrapper.jar").is_file());
            let props = std::fs::read_to_string(dir.join("gradle/wrapper/gradle-wrapper.properties")).unwrap();
            assert!(props.contains(GRADLE_VERSION));
            // The app reads the Java level the build asks for, to pick a JDK for it.
            assert_eq!(crate::lang::java::requirement(&*snout_host::local(), &dir).declared, Some(21));
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn writes_a_spring_boot_project_offline_with_the_known_dependencies() {
        let dir = std::env::temp_dir().join(format!("snout-new-boot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let boot = Boot { version: "4.1.2".into(), dependencies: vec!["web".into(), "lombok".into(), "kafka".into()] };
        write_boot_offline(&*snout_host::local(), &dir, &basics(Dsl::Kotlin), &boot).unwrap();
        let build = std::fs::read_to_string(dir.join("build.gradle.kts")).unwrap();
        assert!(build.contains("id(\"org.springframework.boot\") version \"4.1.2\""), "{build}");
        assert!(build.contains("spring-boot-starter-webmvc-test\""), "{build}");
        assert!(build.contains("annotationProcessor(\"org.projectlombok:lombok\")"), "{build}");
        assert!(!build.contains("kafka"), "an unknown dependency is left out");
        assert!(dir.join("src/main/java/com/example/myshop/MyShopApplication.java").is_file());
        assert!(dir.join("src/test/java/com/example/myshop/MyShopApplicationTests.java").is_file());
        let props = std::fs::read_to_string(dir.join("gradle/wrapper/gradle-wrapper.properties")).unwrap();
        assert!(props.contains(BOOT_GRADLE_VERSION), "Spring Boot 4 needs a newer Gradle");
        let _ = std::fs::remove_dir_all(dir);
        let url = starter_url("https://start.spring.io", &basics(Dsl::Groovy), &boot);
        assert!(url.contains("type=gradle-project&"), "{url}");
        assert!(url.contains("dependencies=web,lombok,kafka"), "{url}");
        assert!(url.contains("javaVersion=21"), "{url}");
    }

    /// With `SNOUT_NEW_PROJECTS=<dir>`: a Java project in each DSL, a Spring Boot one made
    /// here and one from start.spring.io, written there to build by hand.
    #[test]
    fn writes_projects_to_build_by_hand() {
        let Some(out) = std::env::var_os("SNOUT_NEW_PROJECTS").map(PathBuf::from) else { return };
        let boot = Boot { version: OFFLINE_BOOT.into(), dependencies: vec!["web".into(), "lombok".into()] };
        let mut b = basics(Dsl::Kotlin);
        write_java(&*snout_host::local(), &out.join("java-kotlin"), &b).unwrap();
        b.dsl = Dsl::Groovy;
        b.java = 8;
        write_java(&*snout_host::local(), &out.join("java-groovy-8"), &b).unwrap();
        b.java = 17;
        write_boot_offline(&*snout_host::local(), &out.join("boot-offline"), &b, &boot).unwrap();
        let catalog =
            crate::new_project::initializr::fetch(crate::new_project::initializr::START_SPRING_IO, None).unwrap();
        assert!(catalog.dependency("web").is_some() && !catalog.boot.is_empty(), "{catalog:?}");
        b.name = "boot-online".into();
        download_boot(
            &*snout_host::local(),
            crate::new_project::initializr::START_SPRING_IO,
            &out.join("boot-online"),
            &b,
            &boot,
        )
        .unwrap();
    }
}
