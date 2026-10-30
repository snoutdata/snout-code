"""sample.py <dir> gradle|maven <tool version> <java level>: a two-module sample build for
scripts/compat-matrix.sh. A library and an application that uses it, a JUnit 5 test, and code of
the Java level: sealed types and records from 17, pattern switches from 21, a statement before
super() from 25. The Gradle build has only its wrapper's properties (the Tooling API reads them);
the Maven build gets its wrapper from the matrix script."""
import os, sys

out, tool, version, level = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])

def w(path, text):
    p = os.path.join(out, path)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    open(p, "w").write(text.lstrip("\n"))

greeter = """
package demo.lib;

/** Says hello. */
public final class Greeter {
    private final String name;

    public Greeter(String name) {
        this.name = name;
    }

    public String name() {
        return name;
    }

    public String greet() {
        return "Hello, " + name + "!";
    }
}
"""
if level >= 17:
    w("lib/src/main/java/demo/lib/Shape.java", """
package demo.lib;

/** Sealed types and records: Java 17. */
public sealed interface Shape permits Shape.Circle, Shape.Square {
    record Circle(double radius) implements Shape {}

    record Square(double side) implements Shape {}
}
""")
describe = "        return o instanceof Greeter ? \"a greeter\" : \"something\";\n"
if level >= 21:
    describe = """        return switch (o) {
            case Greeter g when g.name().isEmpty() -> "a greeter of nobody";
            case Greeter g -> "a greeter of " + g.name();
            case demo.lib.Shape.Circle(double r) -> "a circle of " + r;
            default -> "something";
        };
"""
extra = ""
if level >= 25:
    extra = """
    /** A statement before super(): flexible constructor bodies, Java 25. */
    static final class Checked {
        final int n;

        Checked(int n) {
            if (n < 0) {
                throw new IllegalArgumentException("negative");
            }
            super();
            this.n = n;
        }
    }
"""
w("lib/src/main/java/demo/lib/Greeter.java", greeter)
w("app/src/main/java/demo/app/App.java", """
package demo.app;

import demo.lib.Greeter;

public final class App {
    public static void main(String[] args) {
        Greeter greeter = new Greeter("SnoutCode");
        System.out.println(greeter.greet());
        System.out.println(describe(greeter));
    }

    static String describe(Object o) {
%s    }
%s}
""" % (describe, extra))
w("app/src/test/java/demo/app/AppTest.java", """
package demo.app;

import static org.junit.jupiter.api.Assertions.assertEquals;

import demo.lib.Greeter;
import org.junit.jupiter.api.Test;

class AppTest {
    @Test
    void greets() {
        assertEquals("Hello, test!", new Greeter("test").greet());
    }
}
""")

if tool == "gradle":
    w("settings.gradle", "rootProject.name = 'sample'\ninclude 'lib', 'app'\n")
    w("build.gradle", """
subprojects {
    apply plugin: 'java'
    repositories { mavenCentral() }
    tasks.withType(JavaCompile).configureEach { options.release.set(%d) }
    dependencies {
        testImplementation platform('org.junit:junit-bom:5.11.4')
        testImplementation 'org.junit.jupiter:junit-jupiter'
        testRuntimeOnly 'org.junit.platform:junit-platform-launcher'
    }
    tasks.withType(Test).configureEach { useJUnitPlatform() }
}
project(':lib') {
    apply plugin: 'java-library'
}
project(':app') {
    apply plugin: 'application'
    dependencies { implementation project(':lib') }
    application { mainClass.set('demo.app.App') }
}
""" % level)
    w("gradle/wrapper/gradle-wrapper.properties",
      "distributionUrl=https\\://services.gradle.org/distributions/gradle-%s-bin.zip\n" % version)
else:
    def pom(artifact, body, packaging="jar", parent=True):
        head = """<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
"""
        par = """  <parent><groupId>demo</groupId><artifactId>sample</artifactId><version>1.0</version></parent>
""" if parent else "  <groupId>demo</groupId>\n  <version>1.0</version>\n"
        return head + par + "  <artifactId>%s</artifactId>\n  <packaging>%s</packaging>\n%s</project>\n" % (artifact, packaging, body)
    w("pom.xml", pom("sample", """  <modules><module>lib</module><module>app</module></modules>
  <properties>
    <maven.compiler.release>%d</maven.compiler.release>
    <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
  </properties>
  <dependencyManagement><dependencies>
    <dependency><groupId>org.junit</groupId><artifactId>junit-bom</artifactId><version>5.11.4</version><type>pom</type><scope>import</scope></dependency>
  </dependencies></dependencyManagement>
  <dependencies>
    <dependency><groupId>org.junit.jupiter</groupId><artifactId>junit-jupiter</artifactId><scope>test</scope></dependency>
  </dependencies>
  <build><pluginManagement><plugins>
    <plugin><artifactId>maven-compiler-plugin</artifactId><version>3.14.0</version></plugin>
    <plugin><artifactId>maven-surefire-plugin</artifactId><version>3.5.2</version></plugin>
    <plugin><artifactId>maven-jar-plugin</artifactId><version>3.4.2</version></plugin>
    <plugin><artifactId>maven-resources-plugin</artifactId><version>3.3.1</version></plugin>
    <plugin><artifactId>maven-install-plugin</artifactId><version>3.1.3</version></plugin>
  </plugins></pluginManagement></build>
""" % level, packaging="pom", parent=False))
    w("lib/pom.xml", pom("lib", ""))
    w("app/pom.xml", pom("app", """  <dependencies>
    <dependency><groupId>demo</groupId><artifactId>lib</artifactId><version>1.0</version></dependency>
  </dependencies>
"""))
