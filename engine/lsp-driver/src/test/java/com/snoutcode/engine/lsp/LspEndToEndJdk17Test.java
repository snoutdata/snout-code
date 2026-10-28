package com.snoutcode.engine.lsp;

/**
 * The end-to-end test with the fixture's Gradle on Java 17, as most projects run it, while jdtls
 * runs on Java 21. The JDT projects then get a JDK that is not jdtls's own, which once left their
 * compiler settings unsaved: the search engine used jdtls's Java 21 compliance against the Java 17
 * library, and find references on a method came back empty.
 */
class LspEndToEndJdk17Test extends LspEndToEndTest {
    @Override
    String gradleJavaHome() {
        return System.getProperty("snout.test.jdk17");
    }
}
