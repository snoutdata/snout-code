package com.snoutcode.engine.spring;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * What one compilation unit (or one class folder of Scala classes, or one XML file) says about
 * the Spring application: its bean declarations, injection points, endpoints and scan settings.
 * Facts hold names, never JDT handles or bindings, so they outlive the AST they came from and
 * the resolver can work on them without Eclipse.
 */
final class Facts {
    final List<BeanDef> beans = new ArrayList<>();
    /** Interfaces, which become beans when Spring Data or MyBatis registers them. */
    final List<BeanDef> interfaces = new ArrayList<>();
    final List<InjectionPoint> injections = new ArrayList<>();
    final List<Endpoint> endpoints = new ArrayList<>();
    final List<ScanSpec> scans = new ArrayList<>();
    /** Importing class to the classes it imports with {@code @Import}, through meta-annotations. */
    final Map<String, Set<String>> imports = new LinkedHashMap<>();
    /** Importing class to the XML files it imports with {@code @ImportResource}. */
    final Map<String, Set<String>> importResources = new LinkedHashMap<>();
    /** Classes named by {@code @EnableConfigurationProperties}. */
    final Set<String> enabledProperties = new LinkedHashSet<>();
    /** Property keys the code reads: {@code @Value}, {@code @ConditionalOnProperty}, {@code getProperty}. */
    final List<KeyUse> keyUses = new ArrayList<>();
    /** Properties of {@code @ConfigurationProperties} classes declared here. */
    final List<SourceProperty> properties = new ArrayList<>();
    /** Every class declared here that is a configuration class (for nested configurations). */
    final Map<String, String> enclosing = new HashMap<>();

    boolean isEmpty() {
        return beans.isEmpty() && interfaces.isEmpty() && injections.isEmpty() && endpoints.isEmpty() && scans.isEmpty()
            && imports.isEmpty() && importResources.isEmpty() && keyUses.isEmpty() && properties.isEmpty()
            && enabledProperties.isEmpty();
    }

    /** A bean definition: a component class, a {@code @Bean} method, an XML bean, a registered interface. */
    static final class BeanDef {
        String name;
        final List<String> aliases = new ArrayList<>();
        /** Binary name of the bean's type ({@code a.B$C}). */
        String type;
        /** Binary names of the type and all its supertypes, and of a FactoryBean's product. */
        final Set<String> assignable = new LinkedHashSet<>();
        /** Supertype binary name to its parameterized form, where it has concrete type arguments. */
        final Map<String, String> parameterized = new HashMap<>();
        /** component, service, repository, controller, restController, configuration, bean, xml, dataRepository, mapper, properties, library. */
        String kind;
        /** The annotation that made the class a bean, as written on it. */
        String stereotype;
        /** The component class, or the configuration class of a {@code @Bean} method. */
        String declaringType;
        String method;
        String packageName;
        Loc loc;
        String language;
        String project;
        boolean primary;
        boolean fallback;
        boolean autowireCandidate = true;
        boolean isAbstract;
        boolean isInterface;
        /** Any annotation on an interface: a library may register a proxy for it (Feign, MyBatis). */
        boolean annotatedInterface;
        boolean configuration;
        final List<String> qualifiers = new ArrayList<>();
        final List<String> profiles = new ArrayList<>();
        final List<String> conditions = new ArrayList<>();

        boolean conditional() {
            return !profiles.isEmpty() || !conditions.isEmpty() || "library".equals(kind);
        }

        String id() {
            return method != null ? declaringType + "#" + method : kind + ":" + (declaringType != null ? declaringType : type) + ":" + name;
        }

        JsonObject toJson() {
            JsonObject o = new JsonObject();
            o.addProperty("name", name);
            if (!aliases.isEmpty()) {
                o.add("aliases", strings(aliases));
            }
            o.addProperty("type", Names.display(type));
            o.addProperty("kind", kind);
            if (stereotype != null) {
                o.addProperty("stereotype", stereotype);
            }
            if (declaringType != null) {
                o.addProperty("declaringType", Names.display(declaringType));
            }
            if (method != null) {
                o.addProperty("method", method);
            }
            if (loc != null) {
                o.addProperty("uri", loc.uri);
                o.add("range", loc.range());
            }
            o.addProperty("language", language);
            if (primary) {
                o.addProperty("primary", true);
            }
            if (!qualifiers.isEmpty()) {
                o.add("qualifiers", strings(qualifiers));
            }
            if (!profiles.isEmpty()) {
                o.add("profiles", strings(profiles));
            }
            if (!conditions.isEmpty()) {
                o.add("conditions", strings(conditions));
            }
            return o;
        }
    }

    /** Somewhere Spring injects a value: a field, a constructor, setter or {@code @Bean} method parameter. */
    static final class InjectionPoint {
        /** field, constructor, setter, method, beanMethod. */
        String kind;
        /** autowired, inject, resource, value, implicit (a constructor without annotation). */
        String annotation;
        String declaringType;
        /** The method or constructor (its simple name); null for a field. */
        String member;
        /** The field or parameter name, when known. */
        String name;
        /** Binary name of the type to inject; for a collection, map, Optional or provider, of its element. */
        String type;
        /** The element type with its type arguments, for generic matching. */
        String fullType;
        /** Binary names of the type and its supertypes: a {@code @Bean} method declared as a supertype may still match. */
        final Set<String> typeSupers = new LinkedHashSet<>();
        /** null, collection, map, optional, provider, array. */
        String wrapper;
        boolean required = true;
        boolean unresolved;
        final List<String> qualifiers = new ArrayList<>();
        /** {@code @Resource}'s name. */
        String resourceName;
        /** {@code @Value}'s expression. */
        String value;
        Loc loc;
        String language;
        String project;
        /** Quick fix data: where {@code @Qualifier} goes, how the annotation reads, the import. */
        JsonObject fix;

        // Resolution.
        final List<BeanDef> beans = new ArrayList<>();
        /** resolved, missing, ambiguous, multiple (a collection), optional, value, framework, unresolved, unknown. */
        String status;
        String message;

        String typeDisplay() {
            return Names.simple(fullType != null ? fullType : type);
        }
    }

    /** A request handler. */
    static final class Endpoint {
        final List<String> paths = new ArrayList<>();
        final List<String> methods = new ArrayList<>();
        final List<String> produces = new ArrayList<>();
        final List<String> consumes = new ArrayList<>();
        final List<String> params = new ArrayList<>();
        final List<String> headers = new ArrayList<>();
        String controller;
        String declaringType;
        String handler;
        Loc loc;
        String language;
        String project;

        JsonObject toJson(String module) {
            JsonObject o = new JsonObject();
            if (module != null) {
                o.addProperty("module", module);
            }
            o.addProperty("path", paths.isEmpty() ? "/" : paths.get(0));
            o.add("paths", strings(paths.isEmpty() ? List.of("/") : paths));
            o.add("methods", strings(methods));
            if (!produces.isEmpty()) {
                o.add("produces", strings(produces));
            }
            if (!consumes.isEmpty()) {
                o.add("consumes", strings(consumes));
            }
            if (!params.isEmpty()) {
                o.add("params", strings(params));
            }
            if (!headers.isEmpty()) {
                o.add("headers", strings(headers));
            }
            JsonObject h = new JsonObject();
            h.addProperty("class", Names.display(controller));
            if (!controller.equals(declaringType)) {
                h.addProperty("declaringClass", Names.display(declaringType));
            }
            h.addProperty("method", handler);
            o.add("handler", h);
            if (loc != null) {
                o.addProperty("uri", loc.uri);
                o.add("range", loc.range());
            }
            o.addProperty("language", language);
            return o;
        }
    }

    /** Packages a class or XML file scans: for components, MyBatis mappers. */
    static final class ScanSpec {
        /** componentScan, mapperScan. */
        String kind;
        final List<String> packages = new ArrayList<>();
        String declaringType;
        /** Whether it starts a Spring Boot application ({@code @SpringBootApplication}, {@code @EnableAutoConfiguration}). */
        boolean application;
        Loc loc;
        String project;
    }

    /** A property key the code reads. */
    static final class KeyUse {
        String key;
        String defaultValue;
        Loc loc;
        /** The string literal holding it, for inlay hints: the range of the whole literal. */
        Loc literal;
    }

    /** A property of a {@code @ConfigurationProperties} class in the sources. */
    static final class SourceProperty {
        String name;
        String type;
        String description;
        String defaultValue;
        String sourceType;
        boolean deprecated;
        /** For a Map or a nested class bound as one: keys go below the name. */
        boolean map;
        Loc loc;
    }

    static JsonArray strings(Iterable<String> values) {
        JsonArray a = new JsonArray();
        values.forEach(a::add);
        return a;
    }
}
