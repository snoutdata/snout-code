package com.snoutcode.engine.spring;

import com.google.gson.JsonObject;
import com.snoutcode.engine.spring.Annotations.Found;
import com.snoutcode.engine.spring.Facts.BeanDef;
import com.snoutcode.engine.spring.Facts.Endpoint;
import com.snoutcode.engine.spring.Facts.InjectionPoint;
import com.snoutcode.engine.spring.Facts.KeyUse;
import com.snoutcode.engine.spring.Facts.ScanSpec;
import com.snoutcode.engine.spring.Facts.SourceProperty;
import org.eclipse.jdt.core.dom.IAnnotationBinding;
import org.eclipse.jdt.core.dom.IBinding;
import org.eclipse.jdt.core.dom.IMethodBinding;
import org.eclipse.jdt.core.dom.ITypeBinding;
import org.eclipse.jdt.core.dom.IVariableBinding;
import org.eclipse.jdt.core.dom.Modifier;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Reads the Spring facts of a type from its JDT bindings: whether it is a bean, its
 * {@code @Bean} methods, injection points, request mappings, scans, imports and configuration
 * properties. Bindings look the same for source and class files, so the Scala bridge's classes
 * go through here too; only where things are differs, which {@link Positions} answers.
 */
final class TypeScanner {
    /** Where the parts of a type are in its file. */
    interface Positions {
        Loc type(ITypeBinding t);

        Loc field(IVariableBinding f);

        Loc method(IMethodBinding m);

        Loc param(IMethodBinding m, int index);

        /** The string literal of an annotation on a field ({@code param < 0}, {@code element} a field), method or parameter. */
        default Loc literal(IBinding element, int param, String annotation) {
            return null;
        }

        /** Quick fix data for an injection point; null when there is none. */
        default JsonObject fix(IBinding element, int param) {
            return null;
        }

        default String javadoc(IBinding member) {
            return null;
        }

        default String initializer(IVariableBinding field) {
            return null;
        }
    }

    private static final Set<String> COLLECTIONS = Set.of("java.util.Collection", "java.util.List", "java.util.Set",
        "java.util.SortedSet", "java.util.NavigableSet");
    private static final Set<String> MAPS = Set.of("java.util.Map", "java.util.SortedMap", "java.util.NavigableMap");
    private static final Set<String> PROVIDERS = Set.of("org.springframework.beans.factory.ObjectProvider",
        "org.springframework.beans.factory.ObjectFactory", "jakarta.inject.Provider", "javax.inject.Provider");

    private final Facts out;
    private final Positions pos;
    private final String project;
    private final String language;

    TypeScanner(Facts out, Positions pos, String project, String language) {
        this.out = out;
        this.pos = pos;
        this.project = project;
        this.language = language;
    }

    /** Scans a type and its member types. */
    void scan(ITypeBinding type) {
        if (type == null || type.isRecovered()) {
            return;
        }
        try {
            scanType(type);
        } catch (RuntimeException e) {
            // A binding JDT cannot complete (a missing class): skip the type, keep the rest.
        }
        for (ITypeBinding member : type.getDeclaredTypes()) {
            if (Modifier.isStatic(member.getModifiers()) || member.isInterface() || member.isEnum() || member.isRecord()) {
                out.enclosing.put(binary(member), binary(type));
            }
            scan(member);
        }
    }

    private void scanType(ITypeBinding type) {
        if (type.isAnnotation() || type.isAnonymous() || type.isLocal()) {
            return;
        }
        String binaryName = binary(type);
        IAnnotationBinding[] annotations = type.getAnnotations();
        if (type.isInterface()) {
            BeanDef i = new BeanDef();
            i.name = Names.beanName(binaryName);
            i.type = binaryName;
            i.kind = Annotations.direct(annotations, Annotations.MAPPER) != null ? "mapper" : "interface";
            i.declaringType = binaryName;
            i.packageName = type.getPackage() == null ? "" : type.getPackage().getName();
            i.isInterface = true;
            i.annotatedInterface = annotations.length > 0;
            if (Annotations.direct(annotations, Annotations.NO_REPOSITORY_BEAN) != null) {
                i.conditions.add(Annotations.NO_REPOSITORY_BEAN);
            }
            supertypes(type, i.assignable, i.parameterized);
            i.loc = pos.type(type);
            i.language = language;
            i.project = project;
            qualifiersOf(annotations, i.qualifiers);
            i.primary = Annotations.direct(annotations, Annotations.PRIMARY) != null;
            // Spring Data and MyBatis may register any interface: the resolver decides.
            out.interfaces.add(i);
            return;
        }
        if (type.isEnum()) {
            return;
        }
        boolean isAbstract = Modifier.isAbstract(type.getModifiers());
        boolean innerInstance = type.isMember() && !Modifier.isStatic(type.getModifiers());
        Found stereotype = Annotations.meta(annotations, Annotations.COMPONENT);
        IAnnotationBinding named = Annotations.direct(annotations, Annotations.NAMED);
        IAnnotationBinding props = Annotations.direct(annotations, Annotations.CONFIGURATION_PROPERTIES);
        BeanDef bean = null;
        if ((stereotype != null || named != null || props != null) && !innerInstance) {
            bean = new BeanDef();
            bean.kind = stereotype != null ? kind(annotations) : named != null ? "component" : "properties";
            IAnnotationBinding top = stereotype != null ? stereotype.top() : named != null ? named : props;
            bean.stereotype = Annotations.name(top);
            String explicit = stereotype != null || named != null ? firstString(top, "value") : null;
            bean.name = explicit != null && !explicit.isEmpty() ? explicit : Names.beanName(binaryName);
            bean.type = binaryName;
            bean.declaringType = binaryName;
            bean.packageName = type.getPackage() == null ? "" : type.getPackage().getName();
            bean.isAbstract = isAbstract;
            bean.configuration = Annotations.meta(annotations, Annotations.CONFIGURATION) != null;
            supertypes(type, bean.assignable, bean.parameterized);
            factoryProduct(type, bean);
            conditions(annotations, bean);
            bean.loc = pos.type(type);
            bean.language = language;
            bean.project = project;
            out.beans.add(bean);
        }
        scans(type, annotations, binaryName);
        imports(annotations, binaryName);
        if (props != null) {
            properties(type, prefix(props), new HashSet<>(), 0);
        }
        conditionKeys(annotations);

        for (IVariableBinding f : type.getDeclaredFields()) {
            if (!Modifier.isStatic(f.getModifiers())) {
                field(type, f);
            }
        }
        List<IMethodBinding> constructors = new ArrayList<>();
        for (IMethodBinding m : type.getDeclaredMethods()) {
            if (m.isConstructor()) {
                if (!m.isSynthetic()) {
                    constructors.add(m);
                }
                continue;
            }
            method(type, m, bean);
        }
        boolean constructorInjection = bean != null && !isAbstract && !"properties".equals(bean.kind);
        IMethodBinding chosen = null;
        for (IMethodBinding c : constructors) {
            IAnnotationBinding[] ca = c.getAnnotations();
            if (Annotations.direct(ca, Annotations.AUTOWIRED) != null || Annotations.direct(ca, Annotations.INJECT) != null) {
                chosen = c;
            }
        }
        if (chosen == null && constructorInjection && constructors.size() == 1) {
            chosen = constructors.get(0);
        }
        if (chosen != null && (constructorInjection || bean != null)) {
            IAnnotationBinding[] ca = chosen.getAnnotations();
            String annotation = Annotations.direct(ca, Annotations.AUTOWIRED) != null ? "autowired"
                : Annotations.direct(ca, Annotations.INJECT) != null ? "inject" : "implicit";
            boolean required = Annotations.booleanValue(Annotations.direct(ca, Annotations.AUTOWIRED), "required", true);
            parameters(type, chosen, "constructor", annotation, required);
        }
        if (bean != null && Annotations.meta(annotations, Annotations.CONTROLLER) != null && !isAbstract) {
            endpoints(type);
        }
    }

    private static String kind(IAnnotationBinding[] annotations) {
        if (Annotations.meta(annotations, Annotations.REST_CONTROLLER) != null) {
            return "restController";
        }
        if (Annotations.meta(annotations, Annotations.CONTROLLER) != null) {
            return "controller";
        }
        if (Annotations.meta(annotations, Annotations.CONFIGURATION) != null) {
            return "configuration";
        }
        if (Annotations.meta(annotations, Annotations.SERVICE) != null) {
            return "service";
        }
        if (Annotations.meta(annotations, Annotations.REPOSITORY) != null) {
            return "repository";
        }
        return "component";
    }

    // ---- scans, imports, keys

    private void scans(ITypeBinding type, IAnnotationBinding[] annotations, String binaryName) {
        String own = type.getPackage() == null ? "" : type.getPackage().getName();
        IAnnotationBinding app = Annotations.direct(annotations, Annotations.BOOT_APPLICATION);
        Found auto = Annotations.meta(annotations, Annotations.ENABLE_AUTO_CONFIGURATION);
        if (app != null || auto != null) {
            ScanSpec s = spec("componentScan", binaryName, type);
            s.application = true;
            if (app != null) {
                s.packages.addAll(Annotations.strings(Annotations.value(app, "scanBasePackages")));
                for (ITypeBinding t : Annotations.types(Annotations.value(app, "scanBasePackageClasses"))) {
                    s.packages.add(t.getPackage() == null ? "" : t.getPackage().getName());
                }
            }
            if (s.packages.isEmpty()) {
                s.packages.add(own);
            }
            out.scans.add(s);
        }
        List<IAnnotationBinding> componentScans = new ArrayList<>();
        IAnnotationBinding direct = Annotations.direct(annotations, Annotations.COMPONENT_SCAN);
        if (direct != null) {
            componentScans.add(direct);
        }
        IAnnotationBinding container = Annotations.direct(annotations, Annotations.COMPONENT_SCANS);
        if (container != null) {
            componentScans.addAll(Annotations.annotations(Annotations.value(container, "value")));
        }
        for (IAnnotationBinding a : componentScans) {
            ScanSpec s = spec("componentScan", binaryName, type);
            s.packages.addAll(Annotations.firstStrings(a, "value", "basePackages"));
            for (ITypeBinding t : Annotations.types(Annotations.value(a, "basePackageClasses"))) {
                s.packages.add(t.getPackage() == null ? "" : t.getPackage().getName());
            }
            if (s.packages.isEmpty()) {
                s.packages.add(own);
            }
            out.scans.add(s);
        }
        IAnnotationBinding mapperScan = Annotations.direct(annotations, Annotations.MAPPER_SCAN);
        if (mapperScan != null) {
            ScanSpec s = spec("mapperScan", binaryName, type);
            s.packages.addAll(Annotations.firstStrings(mapperScan, "value", "basePackages"));
            for (ITypeBinding t : Annotations.types(Annotations.value(mapperScan, "basePackageClasses"))) {
                s.packages.add(t.getPackage() == null ? "" : t.getPackage().getName());
            }
            if (s.packages.isEmpty()) {
                s.packages.add(own);
            }
            out.scans.add(s);
        }
    }

    private ScanSpec spec(String kind, String declaringType, ITypeBinding type) {
        ScanSpec s = new ScanSpec();
        s.kind = kind;
        s.declaringType = declaringType;
        s.loc = pos.type(type);
        s.project = project;
        return s;
    }

    /** {@code @Import}, {@code @ImportResource} and {@code @EnableConfigurationProperties}, through meta-annotations. */
    private void imports(IAnnotationBinding[] annotations, String binaryName) {
        for (IAnnotationBinding a : Annotations.all(annotations)) {
            String n = Annotations.name(a);
            if (Annotations.IMPORT.equals(n)) {
                for (ITypeBinding t : Annotations.types(Annotations.value(a, "value"))) {
                    out.imports.computeIfAbsent(binaryName, k -> new LinkedHashSet<>()).add(binary(t));
                }
            } else if (Annotations.IMPORT_RESOURCE.equals(n)) {
                for (String location : Annotations.firstStrings(a, "value", "locations")) {
                    out.importResources.computeIfAbsent(binaryName, k -> new LinkedHashSet<>()).add(location);
                }
            } else if (Annotations.ENABLE_CONFIGURATION_PROPERTIES.equals(n)) {
                for (ITypeBinding t : Annotations.types(Annotations.value(a, "value"))) {
                    out.enabledProperties.add(binary(t));
                }
            }
        }
    }

    private void conditionKeys(IAnnotationBinding[] annotations) {
        IAnnotationBinding a = Annotations.direct(annotations, Annotations.CONDITIONAL_ON_PROPERTY);
        if (a == null) {
            return;
        }
        String prefix = firstString(a, "prefix");
        for (String name : Annotations.firstStrings(a, "name", "value")) {
            KeyUse k = new KeyUse();
            k.key = prefix == null || prefix.isEmpty() ? name : prefix.replaceAll("\\.$", "") + "." + name;
            out.keyUses.add(k);
        }
    }

    // ---- members

    private void field(ITypeBinding owner, IVariableBinding f) {
        IAnnotationBinding[] annotations = f.getAnnotations();
        IAnnotationBinding value = Annotations.direct(annotations, Annotations.VALUE);
        IAnnotationBinding autowired = Annotations.direct(annotations, Annotations.AUTOWIRED);
        IAnnotationBinding inject = Annotations.direct(annotations, Annotations.INJECT);
        IAnnotationBinding resource = Annotations.direct(annotations, Annotations.RESOURCE);
        if (value == null && autowired == null && inject == null && resource == null) {
            return;
        }
        InjectionPoint i = point(owner, "field", null, f.getName(), f.getType(), annotations);
        i.loc = pos.field(f);
        if (value != null) {
            value(i, value, pos.literal(f, -1, Annotations.VALUE));
        } else if (resource != null) {
            resource(i, resource, f.getName());
        } else {
            i.annotation = autowired != null ? "autowired" : "inject";
            i.required &= Annotations.booleanValue(autowired, "required", true);
        }
        i.fix = pos.fix(f, -1);
        out.injections.add(i);
    }

    private void method(ITypeBinding owner, IMethodBinding m, BeanDef component) {
        IAnnotationBinding[] annotations = m.getAnnotations();
        Found bean = Annotations.meta(annotations, Annotations.BEAN);
        if (bean != null && !Modifier.isAbstract(m.getModifiers())) {
            beanMethod(owner, m, bean.top(), annotations, component);
            parameters(owner, m, "beanMethod", "implicit", true);
            return;
        }
        if (Annotations.direct(annotations, Annotations.AUTOWIRED) != null || Annotations.direct(annotations, Annotations.INJECT) != null) {
            boolean required = Annotations.booleanValue(Annotations.direct(annotations, Annotations.AUTOWIRED), "required", true);
            String kind = m.getName().startsWith("set") && m.getParameterTypes().length == 1 ? "setter" : "method";
            parameters(owner, m, kind, Annotations.direct(annotations, Annotations.AUTOWIRED) != null ? "autowired" : "inject", required);
            return;
        }
        IAnnotationBinding resource = Annotations.direct(annotations, Annotations.RESOURCE);
        if (resource != null && m.getParameterTypes().length == 1) {
            String property = m.getName().startsWith("set") ? Names.decapitalize(m.getName().substring(3)) : m.getName();
            InjectionPoint i = point(owner, "setter", m.getName(), property, m.getParameterTypes()[0], m.getParameterAnnotations(0));
            resource(i, resource, property);
            i.loc = pos.method(m);
            out.injections.add(i);
            return;
        }
        IAnnotationBinding value = Annotations.direct(annotations, Annotations.VALUE);
        if (value != null && m.getParameterTypes().length == 1) {
            InjectionPoint i = point(owner, "setter", m.getName(), null, m.getParameterTypes()[0], annotations);
            value(i, value, pos.literal(m, -1, Annotations.VALUE));
            i.loc = pos.method(m);
            out.injections.add(i);
        }
    }

    private void beanMethod(ITypeBinding owner, IMethodBinding m, IAnnotationBinding beanAnnotation, IAnnotationBinding[] annotations,
                            BeanDef component) {
        ITypeBinding returnType = m.getReturnType();
        if (returnType == null || "void".equals(returnType.getName())) {
            return;
        }
        BeanDef b = new BeanDef();
        b.kind = "bean";
        List<String> names = new ArrayList<>(Annotations.firstStrings(beanAnnotation, "name", "value"));
        names.removeIf(String::isEmpty);
        b.name = names.isEmpty() ? m.getName() : names.get(0);
        if (names.size() > 1) {
            b.aliases.addAll(names.subList(1, names.size()));
        }
        b.type = binary(returnType);
        b.declaringType = binary(owner);
        b.method = m.getName();
        b.packageName = owner.getPackage() == null ? "" : owner.getPackage().getName();
        b.autowireCandidate = Annotations.booleanValue(beanAnnotation, "autowireCandidate", true);
        supertypes(returnType, b.assignable, b.parameterized);
        factoryProduct(returnType, b);
        conditions(annotations, b);
        if (component != null) {
            b.profiles.addAll(component.profiles);
            b.conditions.addAll(component.conditions);
        }
        b.loc = pos.method(m);
        b.language = language;
        b.project = project;
        out.beans.add(b);
        IAnnotationBinding props = Annotations.direct(annotations, Annotations.CONFIGURATION_PROPERTIES);
        if (props != null) {
            properties(returnType, prefix(props), new HashSet<>(), 0);
        }
        conditionKeys(annotations);
    }

    private void parameters(ITypeBinding owner, IMethodBinding m, String kind, String annotation, boolean required) {
        ITypeBinding[] types = m.getParameterTypes();
        String[] names = parameterNames(m);
        for (int p = 0; p < types.length; p++) {
            IAnnotationBinding[] annotations = m.getParameterAnnotations(p);
            InjectionPoint i = point(owner, kind, m.isConstructor() ? owner.getName() : m.getName(), names[p], types[p], annotations);
            i.loc = pos.param(m, p);
            IAnnotationBinding value = Annotations.direct(annotations, Annotations.VALUE);
            if (value != null) {
                value(i, value, pos.literal(m, p, Annotations.VALUE));
            } else {
                i.annotation = annotation;
                i.required &= required;
            }
            i.fix = pos.fix(m, p);
            out.injections.add(i);
        }
    }

    private static String[] parameterNames(IMethodBinding m) {
        String[] names = new String[m.getParameterTypes().length];
        try {
            String[] declared = m.getParameterNames();
            for (int i = 0; i < names.length && declared != null && i < declared.length; i++) {
                names[i] = declared[i] == null || declared[i].matches("arg\\d+") ? null : declared[i];
            }
        } catch (RuntimeException e) {
            // no names in the class file
        }
        return names;
    }

    private InjectionPoint point(ITypeBinding owner, String kind, String member, String name, ITypeBinding declared,
                                 IAnnotationBinding[] annotations) {
        InjectionPoint i = new InjectionPoint();
        i.kind = kind;
        i.declaringType = binary(owner);
        i.member = member;
        i.name = name;
        i.language = language;
        i.project = project;
        qualifiersOf(annotations, i.qualifiers);
        for (IAnnotationBinding a : annotations) {
            String n = Annotations.name(a);
            if (n != null && n.endsWith(".Nullable")) {
                i.required = false;
            }
        }
        target(i, declared);
        return i;
    }

    /** The type to look up: the element of a collection, map, Optional or provider. */
    private static void target(InjectionPoint i, ITypeBinding declared) {
        if (declared == null || declared.isRecovered()) {
            i.unresolved = true;
            return;
        }
        ITypeBinding t = declared;
        if (t.isArray() && t.getDimensions() == 1) {
            i.wrapper = "array";
            t = t.getElementType();
        } else {
            String erasure = t.getErasure().getQualifiedName();
            ITypeBinding[] args = t.getTypeArguments();
            if ("java.util.Optional".equals(erasure)) {
                i.wrapper = "optional";
                t = args.length == 1 ? args[0] : null;
            } else if (PROVIDERS.contains(erasure)) {
                i.wrapper = "provider";
                t = args.length == 1 ? args[0] : null;
            } else if (COLLECTIONS.contains(erasure)) {
                i.wrapper = "collection";
                t = args.length == 1 ? args[0] : null;
            } else if (MAPS.contains(erasure) && args.length == 2 && "java.lang.String".equals(args[0].getQualifiedName())) {
                i.wrapper = "map";
                t = args[1];
            }
        }
        if (t == null || t.isTypeVariable() || t.isWildcardType() || t.isRecovered()) {
            i.unresolved = t == null || t.isRecovered();
            i.type = t != null && t.isWildcardType() && t.getBound() != null ? binary(t.getBound()) : null;
            if (i.type == null && t != null && t.isTypeVariable()) {
                i.unresolved = true;
            }
            return;
        }
        if (t.isPrimitive()) {
            i.type = t.getName();
            return;
        }
        i.type = binary(t);
        i.fullType = t.isParameterizedType() && concrete(t) ? full(t) : i.type;
        supertypes(t, i.typeSupers, new java.util.HashMap<>());
    }

    private void value(InjectionPoint i, IAnnotationBinding value, Loc literal) {
        i.annotation = "value";
        i.value = firstString(value, "value");
        for (Placeholders.Key k : Placeholders.keys(i.value)) {
            KeyUse use = new KeyUse();
            use.key = k.key();
            use.defaultValue = k.defaultValue();
            use.loc = i.loc;
            use.literal = literal;
            out.keyUses.add(use);
        }
    }

    private static void resource(InjectionPoint i, IAnnotationBinding resource, String defaultName) {
        i.annotation = "resource";
        String name = firstString(resource, "name");
        i.resourceName = name == null || name.isEmpty() ? defaultName : name;
        ITypeBinding type = Annotations.types(Annotations.value(resource, "type")).stream().findFirst().orElse(null);
        if (type != null && !"java.lang.Object".equals(type.getQualifiedName())) {
            target(i, type);
        }
    }

    // ---- bean metadata

    private static void qualifiersOf(IAnnotationBinding[] annotations, List<String> out) {
        for (IAnnotationBinding a : annotations) {
            String n = Annotations.name(a);
            if (Annotations.QUALIFIER.equals(n) || Annotations.NAMED.contains(n)) {
                String v = firstString(a, "value");
                if (v != null && !v.isEmpty()) {
                    out.add(v);
                }
            } else if (Annotations.meta(new IAnnotationBinding[] {a}, Annotations.QUALIFIER) != null
                && !Annotations.COMPONENT.equals(n)) {
                out.add("@" + n);
            }
        }
    }

    private static void conditions(IAnnotationBinding[] annotations, BeanDef b) {
        qualifiersOf(annotations, b.qualifiers);
        b.primary |= Annotations.direct(annotations, Annotations.PRIMARY) != null;
        b.fallback |= Annotations.direct(annotations, Annotations.FALLBACK) != null;
        IAnnotationBinding profile = Annotations.direct(annotations, Annotations.PROFILE);
        if (profile != null) {
            b.profiles.addAll(Annotations.strings(Annotations.value(profile, "value")));
        }
        for (IAnnotationBinding a : annotations) {
            String n = Annotations.name(a);
            if (n == null || Annotations.PROFILE.equals(n)) {
                continue;
            }
            if (n.contains(".ConditionalOn") || Annotations.CONDITIONAL.equals(n)
                || Annotations.meta(new IAnnotationBinding[] {a}, Annotations.CONDITIONAL) != null) {
                b.conditions.add("@" + a.getAnnotationType().getName());
            }
        }
    }

    /** A FactoryBean's product is injectable too. */
    private static void factoryProduct(ITypeBinding type, BeanDef b) {
        ITypeBinding factory = find(type, Annotations.FACTORY_BEAN, new HashSet<>());
        if (factory != null && factory.getTypeArguments().length == 1) {
            ITypeBinding product = factory.getTypeArguments()[0];
            if (!product.isTypeVariable() && !product.isWildcardType()) {
                supertypes(product, b.assignable, b.parameterized);
            }
        }
    }

    private static ITypeBinding find(ITypeBinding t, String erasure, Set<String> seen) {
        if (t == null || !seen.add(t.getKey())) {
            return null;
        }
        if (erasure.equals(t.getErasure().getQualifiedName())) {
            return t;
        }
        ITypeBinding found = find(t.getSuperclass(), erasure, seen);
        if (found != null) {
            return found;
        }
        for (ITypeBinding i : t.getInterfaces()) {
            found = find(i, erasure, seen);
            if (found != null) {
                return found;
            }
        }
        return null;
    }

    // ---- request mappings

    private void endpoints(ITypeBinding controller) {
        List<String> classPaths = new ArrayList<>();
        for (ITypeBinding c = controller; c != null && !"java.lang.Object".equals(c.getQualifiedName()); c = c.getSuperclass()) {
            Found mapping = Annotations.meta(c.getAnnotations(), Annotations.REQUEST_MAPPING);
            if (mapping != null) {
                classPaths.addAll(paths(mapping));
                break;
            }
        }
        if (classPaths.isEmpty()) {
            classPaths.add("");
        }
        Set<String> seen = new HashSet<>();
        List<ITypeBinding> chain = new ArrayList<>();
        for (ITypeBinding c = controller; c != null && !"java.lang.Object".equals(c.getQualifiedName()); c = c.getSuperclass()) {
            chain.add(c);
        }
        for (ITypeBinding c : new ArrayList<>(chain)) {
            collectInterfaces(c, chain, new HashSet<>());
        }
        for (ITypeBinding c : chain) {
            for (IMethodBinding m : c.getDeclaredMethods()) {
                if (m.isConstructor() || Modifier.isStatic(m.getModifiers())) {
                    continue;
                }
                Found mapping = Annotations.meta(m.getAnnotations(), Annotations.REQUEST_MAPPING);
                if (mapping == null) {
                    continue;
                }
                StringBuilder sig = new StringBuilder(m.getName());
                for (ITypeBinding p : m.getParameterTypes()) {
                    sig.append(',').append(p.getErasure().getQualifiedName());
                }
                if (!seen.add(sig.toString())) {
                    continue;
                }
                Endpoint e = new Endpoint();
                for (String cp : classPaths) {
                    for (String mp : paths(mapping)) {
                        String combined = combine(cp, mp);
                        if (!e.paths.contains(combined)) {
                            e.paths.add(combined);
                        }
                    }
                }
                IAnnotationBinding top = mapping.top();
                List<String> methods = Annotations.enumNames(Annotations.value(top, "method"));
                if (methods.isEmpty() && mapping.found() != top) {
                    methods = Annotations.enumNames(Annotations.value(mapping.found(), "method"));
                }
                e.methods.addAll(methods);
                e.produces.addAll(attribute(mapping, "produces"));
                e.consumes.addAll(attribute(mapping, "consumes"));
                e.params.addAll(attribute(mapping, "params"));
                e.headers.addAll(attribute(mapping, "headers"));
                e.controller = binary(controller);
                e.declaringType = binary(c);
                e.handler = m.getName();
                e.loc = pos.method(m);
                e.language = language;
                e.project = project;
                out.endpoints.add(e);
            }
        }
    }

    private static void collectInterfaces(ITypeBinding t, List<ITypeBinding> out, Set<String> seen) {
        for (ITypeBinding i : t.getInterfaces()) {
            if (seen.add(i.getKey()) && !out.contains(i)) {
                out.add(i);
                collectInterfaces(i, out, seen);
            }
        }
    }

    private static List<String> attribute(Found mapping, String member) {
        List<String> values = Annotations.strings(Annotations.value(mapping.top(), member));
        if (values.isEmpty() && mapping.found() != mapping.top()) {
            values = Annotations.strings(Annotations.value(mapping.found(), member));
        }
        return values;
    }

    private static List<String> paths(Found mapping) {
        List<String> out = Annotations.firstStrings(mapping.top(), "value", "path");
        if (out.isEmpty() && mapping.found() != mapping.top()) {
            out = Annotations.firstStrings(mapping.found(), "value", "path");
        }
        if (out.isEmpty()) {
            out.add("");
        }
        return out;
    }

    /** A class path and a method path, as Spring joins them. */
    static String combine(String a, String b) {
        String x = a == null ? "" : a.strip();
        String y = b == null ? "" : b.strip();
        if (x.isEmpty() && y.isEmpty()) {
            return "/";
        }
        if (x.isEmpty()) {
            return slash(y);
        }
        if (y.isEmpty()) {
            return slash(x);
        }
        return slash(x).replaceAll("/+$", "") + "/" + y.replaceAll("^/+", "");
    }

    private static String slash(String p) {
        return p.startsWith("/") ? p : "/" + p;
    }

    // ---- configuration properties

    private static String prefix(IAnnotationBinding props) {
        String p = firstString(props, "prefix");
        if (p == null || p.isEmpty()) {
            p = firstString(props, "value");
        }
        return p == null ? "" : p;
    }

    private void properties(ITypeBinding type, String prefix, Set<String> seen, int depth) {
        if (type == null || depth > 6 || !seen.add(type.getKey()) || type.isPrimitive()) {
            return;
        }
        String base = prefix.isEmpty() ? "" : prefix + ".";
        List<IMethodBinding> constructors = new ArrayList<>();
        Set<String> setters = new HashSet<>();
        Set<String> getters = new HashSet<>();
        for (ITypeBinding t = type; t != null && !t.getQualifiedName().startsWith("java."); t = t.getSuperclass()) {
            for (IMethodBinding m : t.getDeclaredMethods()) {
                if (m.isConstructor()) {
                    if (t == type && m.getParameterTypes().length > 0) {
                        constructors.add(m);
                    }
                } else if (m.getName().startsWith("set") && m.getParameterTypes().length == 1) {
                    setters.add(Names.decapitalize(m.getName().substring(3)));
                } else if ((m.getName().startsWith("get") || m.getName().startsWith("is")) && m.getParameterTypes().length == 0) {
                    getters.add(Names.decapitalize(m.getName().substring(m.getName().startsWith("is") ? 2 : 3)));
                }
            }
        }
        if (type.isRecord() || (setters.isEmpty() && constructors.size() == 1)) {
            IMethodBinding c = constructors.isEmpty() ? null : constructors.get(0);
            if (c != null) {
                String[] names = parameterNames(c);
                ITypeBinding[] types = c.getParameterTypes();
                for (int i = 0; i < types.length; i++) {
                    if (names[i] != null) {
                        property(type, base + Names.kebab(names[i]), types[i], pos.param(c, i), null, null, false, seen, depth);
                    }
                }
                return;
            }
        }
        for (ITypeBinding t = type; t != null && !t.getQualifiedName().startsWith("java."); t = t.getSuperclass()) {
            for (IVariableBinding f : t.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers())) {
                    continue;
                }
                ITypeBinding ft = f.getType();
                boolean nested = !isLeaf(ft, f);
                if (!setters.contains(f.getName()) && !(getters.contains(f.getName()) && (nested || isContainer(ft)))) {
                    continue;
                }
                boolean deprecated = Annotations.direct(f.getAnnotations(), "java.lang.Deprecated") != null;
                property(type, base + Names.kebab(f.getName()), ft, pos.field(f), pos.javadoc(f), pos.initializer(f), deprecated, seen, depth);
            }
        }
    }

    private void property(ITypeBinding owner, String name, ITypeBinding type, Loc loc, String description, String defaultValue,
                          boolean deprecated, Set<String> seen, int depth) {
        if (type == null) {
            return;
        }
        if (!isLeaf(type, null) && !isContainer(type)) {
            properties(type, name, seen, depth + 1);
            return;
        }
        SourceProperty p = new SourceProperty();
        p.name = name;
        p.type = type.isPrimitive() ? type.getName() : type.getQualifiedName();
        p.description = description;
        p.defaultValue = defaultValue;
        p.sourceType = binary(owner);
        p.deprecated = deprecated;
        p.map = MAPS.contains(type.getErasure().getQualifiedName()) || "java.util.Properties".equals(type.getQualifiedName());
        p.loc = loc;
        out.properties.add(p);
    }

    private static boolean isContainer(ITypeBinding t) {
        String e = t.getErasure().getQualifiedName();
        return t.isArray() || COLLECTIONS.contains(e) || MAPS.contains(e) || e.startsWith("java.util.");
    }

    /** A type bound from a single value, rather than a nested object of its own properties. */
    private static boolean isLeaf(ITypeBinding t, IVariableBinding field) {
        if (t.isPrimitive() || t.isEnum() || t.isArray() || t.isInterface() || t.isRecovered()) {
            return true;
        }
        String q = t.getErasure().getQualifiedName();
        if (q.startsWith("java.") || q.startsWith("javax.") || q.startsWith("jakarta.")) {
            return true;
        }
        if (q.startsWith("org.springframework.") && (q.endsWith("DataSize") || q.endsWith("Resource") || q.endsWith("MimeType")
            || q.endsWith("MediaType") || q.endsWith("DataUnit"))) {
            return true;
        }
        if (field != null && Annotations.direct(field.getAnnotations(), Annotations.NESTED_CONFIGURATION_PROPERTY) != null) {
            return false;
        }
        return !(t.isFromSource() || t.isMember() || t.isRecord());
    }

    // ---- types

    static String binary(ITypeBinding t) {
        if (t == null) {
            return null;
        }
        if (t.isArray()) {
            return binary(t.getElementType()) + "[]".repeat(t.getDimensions());
        }
        if (t.isPrimitive()) {
            return t.getName();
        }
        ITypeBinding e = t.getErasure();
        String b = e.getBinaryName();
        return b != null ? b : e.getQualifiedName();
    }

    /** The binary names of a type and all its supertypes, and the parameterized forms that have concrete arguments. */
    static void supertypes(ITypeBinding t, Set<String> erasures, Map<String, String> parameterized) {
        if (t == null || t.isPrimitive()) {
            return;
        }
        String b = binary(t);
        if (b == null || "java.lang.Object".equals(b)) {
            return;
        }
        if (t.isParameterizedType() && concrete(t)) {
            parameterized.putIfAbsent(b, full(t));
        }
        if (!erasures.add(b)) {
            return;
        }
        supertypes(t.getSuperclass(), erasures, parameterized);
        for (ITypeBinding i : t.getInterfaces()) {
            supertypes(i, erasures, parameterized);
        }
    }

    private static boolean concrete(ITypeBinding t) {
        for (ITypeBinding a : t.getTypeArguments()) {
            if (a.isTypeVariable() || a.isWildcardType() || a.isCapture() || (a.isParameterizedType() && !concrete(a))) {
                return false;
            }
        }
        return true;
    }

    static String full(ITypeBinding t) {
        StringBuilder s = new StringBuilder(binary(t));
        ITypeBinding[] args = t.getTypeArguments();
        if (args.length > 0) {
            s.append('<');
            for (int i = 0; i < args.length; i++) {
                if (i > 0) {
                    s.append(',');
                }
                s.append(full(args[i]));
            }
            s.append('>');
        }
        return s.toString();
    }

    static String firstString(IAnnotationBinding a, String member) {
        List<String> values = Annotations.strings(Annotations.value(a, member));
        return values.isEmpty() ? null : values.get(0);
    }
}
