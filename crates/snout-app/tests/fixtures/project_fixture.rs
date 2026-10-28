// The project `project-model.json` describes, on disk: included by the library's tests and
// by the screenshots, so both show the same project.

/// Write the fixture project's files under `root` and its library jars under `jars`, and
/// return the model's JSON for them.
#[allow(dead_code)]
pub fn write_project_fixture(root: &std::path::Path, jars: &std::path::Path) -> String {
    let files: &[(&str, &str)] = &[
        ("settings.gradle", "rootProject.name = 'shop'\ninclude 'app'\n"),
        ("build.gradle", "apply from: 'gradle/deps.gradle'\n"),
        ("gradle.properties", "org.gradle.jvmargs=-Xmx1g\n"),
        ("gradle/deps.gradle", "ext.guava = '33.0.0-jre'\n"),
        ("gradle/wrapper/gradle-wrapper.properties", "distributionUrl=gradle-8.10-bin.zip\n"),
        ("app/build.gradle", "plugins { id 'java' }\n"),
        ("app/src/main/java/shop/App.java", "package shop;\n\npublic class App {}\n"),
        ("app/src/main/java/shop/Cart.java", "package shop;\n\npublic class Cart {}\n"),
        ("app/src/main/resources/application.yml", "server:\n  port: 8080\n"),
        ("app/src/test/java/shop/AppTest.java", "package shop;\n\nclass AppTest {}\n"),
        ("app/src/test/resources/fixtures.json", "{}\n"),
        ("app/build/generated/sources/annotationProcessor/java/main/shop/App_.java", "package shop;\n"),
        ("billing/settings.gradle", "rootProject.name = 'billing'\n"),
        ("billing/build.gradle", "apply from: '../gradle/deps.gradle'\n"),
        ("billing/src/main/java/billing/Invoice.java", "package billing;\n\npublic class Invoice {}\n"),
        ("core/src/main/java/core/Money.java", "package core;\n\npublic record Money(long cents) {}\n"),
        ("core/src/main/resources/core.properties", "currency=EUR\n"),
        ("legacy/build.gradle", "apply plugin: 'java'\n"),
        ("legacy/src/main/java/Old.java", "class Old {}\n"),
    ];
    for (path, text) in files {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    std::fs::create_dir_all(jars).unwrap();
    let guava = [
        "META-INF/MANIFEST.MF",
        "com/google/common/base/Strings.class",
        "com/google/common/base/Preconditions.class",
        "com/google/common/base/Strings$1.class",
        "com/google/common/collect/ImmutableList.class",
    ];
    write_fixture_jar(&jars.join("guava-33.0.0-jre.jar"), &guava);
    write_fixture_jar(&jars.join("junit-4.13.2.jar"), &["org/junit/Test.class", "org/junit/Assert.class"]);
    write_fixture_jar(&jars.join("slf4j-api-2.0.9.jar"), &["org/slf4j/Logger.class"]);
    include_str!("project-model.json")
        .replace("$ROOT", &root.to_string_lossy())
        .replace("$JARS", &jars.to_string_lossy())
}

/// A zip of empty entries, stored: enough for a reader of the central directory.
#[allow(dead_code)]
pub fn write_fixture_jar(path: &std::path::Path, names: &[&str]) {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for name in names {
        let offset = out.len() as u32;
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&[0; 12]);
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(name.as_bytes());
        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&(names.len() as u16).to_le_bytes());
    out.extend_from_slice(&(names.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    std::fs::write(path, out).unwrap();
}
