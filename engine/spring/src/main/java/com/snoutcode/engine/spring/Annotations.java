package com.snoutcode.engine.spring;

import org.eclipse.jdt.core.dom.IAnnotationBinding;
import org.eclipse.jdt.core.dom.IMemberValuePairBinding;
import org.eclipse.jdt.core.dom.ITypeBinding;
import org.eclipse.jdt.core.dom.IVariableBinding;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/** Annotations through JDT bindings, meta-annotations included. Works alike for source and class files. */
final class Annotations {
    static final String COMPONENT = "org.springframework.stereotype.Component";
    static final String SERVICE = "org.springframework.stereotype.Service";
    static final String REPOSITORY = "org.springframework.stereotype.Repository";
    static final String CONTROLLER = "org.springframework.stereotype.Controller";
    static final String REST_CONTROLLER = "org.springframework.web.bind.annotation.RestController";
    static final String CONFIGURATION = "org.springframework.context.annotation.Configuration";
    static final String BEAN = "org.springframework.context.annotation.Bean";
    static final String PRIMARY = "org.springframework.context.annotation.Primary";
    static final String FALLBACK = "org.springframework.context.annotation.Fallback";
    static final String PROFILE = "org.springframework.context.annotation.Profile";
    static final String CONDITIONAL = "org.springframework.context.annotation.Conditional";
    static final String IMPORT = "org.springframework.context.annotation.Import";
    static final String IMPORT_RESOURCE = "org.springframework.context.annotation.ImportResource";
    static final String COMPONENT_SCAN = "org.springframework.context.annotation.ComponentScan";
    static final String COMPONENT_SCANS = "org.springframework.context.annotation.ComponentScans";
    static final String QUALIFIER = "org.springframework.beans.factory.annotation.Qualifier";
    static final String AUTOWIRED = "org.springframework.beans.factory.annotation.Autowired";
    static final String VALUE = "org.springframework.beans.factory.annotation.Value";
    static final String BOOT_APPLICATION = "org.springframework.boot.autoconfigure.SpringBootApplication";
    static final String ENABLE_AUTO_CONFIGURATION = "org.springframework.boot.autoconfigure.EnableAutoConfiguration";
    static final String CONFIGURATION_PROPERTIES = "org.springframework.boot.context.properties.ConfigurationProperties";
    static final String ENABLE_CONFIGURATION_PROPERTIES = "org.springframework.boot.context.properties.EnableConfigurationProperties";
    static final String NESTED_CONFIGURATION_PROPERTY = "org.springframework.boot.context.properties.NestedConfigurationProperty";
    static final String DEPRECATED_CONFIGURATION_PROPERTY = "org.springframework.boot.context.properties.DeprecatedConfigurationProperty";
    static final String CONDITIONAL_ON_PROPERTY = "org.springframework.boot.autoconfigure.condition.ConditionalOnProperty";
    static final String REQUEST_MAPPING = "org.springframework.web.bind.annotation.RequestMapping";
    static final String MAPPER = "org.apache.ibatis.annotations.Mapper";
    static final String MAPPER_SCAN = "org.mybatis.spring.annotation.MapperScan";
    static final String NO_REPOSITORY_BEAN = "org.springframework.data.repository.NoRepositoryBean";
    static final String FACTORY_BEAN = "org.springframework.beans.factory.FactoryBean";
    static final Set<String> INJECT = Set.of("jakarta.inject.Inject", "javax.inject.Inject");
    static final Set<String> NAMED = Set.of("jakarta.inject.Named", "javax.inject.Named");
    static final Set<String> RESOURCE = Set.of("jakarta.annotation.Resource", "javax.annotation.Resource");

    private Annotations() {
    }

    /** An annotation found on an element: the one written there, and the one asked for (the same, or a meta-annotation of it). */
    record Found(IAnnotationBinding top, IAnnotationBinding found) {
    }

    static String name(IAnnotationBinding a) {
        ITypeBinding t = a == null ? null : a.getAnnotationType();
        return t == null ? null : t.getErasure().getQualifiedName();
    }

    static IAnnotationBinding direct(IAnnotationBinding[] annotations, String fqn) {
        if (annotations == null) {
            return null;
        }
        for (IAnnotationBinding a : annotations) {
            if (fqn.equals(name(a))) {
                return a;
            }
        }
        return null;
    }

    static IAnnotationBinding direct(IAnnotationBinding[] annotations, Set<String> fqns) {
        if (annotations == null) {
            return null;
        }
        for (IAnnotationBinding a : annotations) {
            if (fqns.contains(name(a))) {
                return a;
            }
        }
        return null;
    }

    /** The annotation {@code fqn} on an element, directly or as a meta-annotation at any depth. */
    static Found meta(IAnnotationBinding[] annotations, String fqn) {
        if (annotations == null) {
            return null;
        }
        for (IAnnotationBinding a : annotations) {
            if (fqn.equals(name(a))) {
                return new Found(a, a);
            }
        }
        for (IAnnotationBinding a : annotations) {
            IAnnotationBinding inner = metaOf(a, fqn, new HashSet<>());
            if (inner != null) {
                return new Found(a, inner);
            }
        }
        return null;
    }

    private static IAnnotationBinding metaOf(IAnnotationBinding a, String fqn, Set<String> seen) {
        String n = name(a);
        if (n == null || !seen.add(n) || n.startsWith("java.lang.annotation.")) {
            return null;
        }
        ITypeBinding type = a.getAnnotationType();
        IAnnotationBinding[] metas;
        try {
            metas = type.getAnnotations();
        } catch (RuntimeException e) {
            return null;
        }
        for (IAnnotationBinding m : metas) {
            if (fqn.equals(name(m))) {
                return m;
            }
        }
        for (IAnnotationBinding m : metas) {
            IAnnotationBinding inner = metaOf(m, fqn, seen);
            if (inner != null) {
                return inner;
            }
        }
        return null;
    }

    /** Every annotation of the element and its meta-annotations. */
    static List<IAnnotationBinding> all(IAnnotationBinding[] annotations) {
        List<IAnnotationBinding> out = new ArrayList<>();
        Set<String> seen = new HashSet<>();
        if (annotations != null) {
            for (IAnnotationBinding a : annotations) {
                collect(a, out, seen, true);
            }
        }
        return out;
    }

    private static void collect(IAnnotationBinding a, List<IAnnotationBinding> out, Set<String> seen, boolean top) {
        String n = name(a);
        if (n == null || n.startsWith("java.lang.annotation.")) {
            return;
        }
        // Every occurrence counts (two @Import meta-annotations), but each type's own annotations are read once.
        out.add(a);
        if (!seen.add(n)) {
            return;
        }
        try {
            for (IAnnotationBinding m : a.getAnnotationType().getAnnotations()) {
                collect(m, out, seen, false);
            }
        } catch (RuntimeException e) {
            // an unresolved annotation type
        }
    }

    /** A member's value as written (declared pairs only), or null. */
    static Object value(IAnnotationBinding a, String member) {
        if (a == null) {
            return null;
        }
        for (IMemberValuePairBinding p : a.getDeclaredMemberValuePairs()) {
            if (member.equals(p.getName())) {
                return p.getValue();
            }
        }
        return null;
    }

    static List<String> strings(Object value) {
        List<String> out = new ArrayList<>();
        if (value instanceof String s) {
            out.add(s);
        } else if (value instanceof Object[] array) {
            for (Object o : array) {
                if (o instanceof String s) {
                    out.add(s);
                }
            }
        }
        return out;
    }

    static List<ITypeBinding> types(Object value) {
        List<ITypeBinding> out = new ArrayList<>();
        if (value instanceof ITypeBinding t) {
            out.add(t);
        } else if (value instanceof Object[] array) {
            for (Object o : array) {
                if (o instanceof ITypeBinding t) {
                    out.add(t);
                }
            }
        }
        return out;
    }

    static List<String> enumNames(Object value) {
        List<String> out = new ArrayList<>();
        if (value instanceof IVariableBinding v) {
            out.add(v.getName());
        } else if (value instanceof Object[] array) {
            for (Object o : array) {
                if (o instanceof IVariableBinding v) {
                    out.add(v.getName());
                }
            }
        }
        return out;
    }

    static List<IAnnotationBinding> annotations(Object value) {
        List<IAnnotationBinding> out = new ArrayList<>();
        if (value instanceof IAnnotationBinding a) {
            out.add(a);
        } else if (value instanceof Object[] array) {
            for (Object o : array) {
                if (o instanceof IAnnotationBinding a) {
                    out.add(a);
                }
            }
        }
        return out;
    }

    /** The strings of the first of these members that is set. */
    static List<String> firstStrings(IAnnotationBinding a, String... members) {
        List<String> out = new ArrayList<>();
        for (String m : members) {
            out.addAll(strings(value(a, m)));
        }
        return out;
    }

    static boolean booleanValue(IAnnotationBinding a, String member, boolean otherwise) {
        Object v = value(a, member);
        return v instanceof Boolean b ? b : otherwise;
    }
}
