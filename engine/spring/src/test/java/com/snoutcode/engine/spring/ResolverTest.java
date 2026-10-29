package com.snoutcode.engine.spring;

import com.snoutcode.engine.spring.Facts.BeanDef;
import com.snoutcode.engine.spring.Facts.InjectionPoint;
import com.snoutcode.engine.spring.Facts.ScanSpec;
import org.junit.jupiter.api.Test;

import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ResolverTest {
    private static final Resolver.Oracle ORACLE = new Resolver.Oracle() {
        @Override
        public Facts xml(String location, Collection<String> projects) {
            if (!location.equals("classpath:beans.xml")) {
                return null;
            }
            return XmlBeans.parse("""
                <beans xmlns="http://www.springframework.org/schema/beans" xmlns:context="http://www.springframework.org/schema/context">
                  <context:component-scan base-package="lib.extra"/>
                  <bean id="restTemplate" class="org.web.RestTemplate"/>
                </beans>""", "file:///beans.xml");
        }

        @Override
        public boolean proxyInterface(String project, String type) {
            return type.equals("app.FeignClient");
        }

        @Override
        public boolean libraryComponent(String project, String type) {
            return false;
        }
    };

    private static BeanDef component(String type, String... supertypes) {
        BeanDef b = new BeanDef();
        b.kind = "service";
        b.type = type;
        b.declaringType = type;
        b.name = Names.beanName(type);
        b.packageName = Names.packageOf(type);
        b.assignable.add(type);
        b.assignable.addAll(List.of(supertypes));
        b.language = "java";
        return b;
    }

    private static InjectionPoint inject(String owner, String name, String type) {
        InjectionPoint i = new InjectionPoint();
        i.kind = "field";
        i.annotation = "autowired";
        i.declaringType = owner;
        i.name = name;
        i.type = type;
        i.loc = new Loc("file:///" + owner + ".java", 1, 0, 1, 4);
        return i;
    }

    private static ScanSpec application(String type, String pkg) {
        ScanSpec s = new ScanSpec();
        s.kind = "componentScan";
        s.application = true;
        s.declaringType = type;
        s.packages.add(pkg);
        return s;
    }

    private static Resolver.ProjectInput project(String name, Facts facts, String... dependencies) {
        Resolver.ProjectInput p = new Resolver.ProjectInput();
        p.name = name;
        p.facts.add(facts);
        p.dependencies.addAll(List.of(dependencies));
        return p;
    }

    @Test
    void resolvesMissingAmbiguousPrimaryAndByName() {
        Facts f = new Facts();
        f.scans.add(application("app.App", "app"));
        f.beans.add(component("app.App"));
        f.beans.add(component("app.English", "app.Greeting"));
        f.beans.add(component("app.French", "app.Greeting"));
        BeanDef primary = component("app.MainStore", "app.Store");
        primary.primary = true;
        f.beans.add(primary);
        f.beans.add(component("app.BackupStore", "app.Store"));
        f.beans.add(component("other.Outside", "app.Audit"));
        InjectionPoint ambiguous = inject("app.App", "greeting", "app.Greeting");
        InjectionPoint byName = inject("app.App", "french", "app.Greeting");
        InjectionPoint store = inject("app.App", "store", "app.Store");
        InjectionPoint missing = inject("app.App", "audit", "app.Audit");
        InjectionPoint optional = inject("app.App", "audit2", "app.Audit");
        optional.required = false;
        InjectionPoint feign = inject("app.App", "client", "app.FeignClient");
        InjectionPoint all = inject("app.App", "greetings", "app.Greeting");
        all.wrapper = "collection";
        f.injections.addAll(List.of(ambiguous, byName, store, missing, optional, feign, all));
        Map<String, Resolver.ProjectInput> projects = new LinkedHashMap<>();
        projects.put("app", project("app", f));
        Resolver.Result r = new Resolver(projects, ORACLE).resolve();
        assertEquals("ambiguous", ambiguous.status);
        assertEquals(2, ambiguous.beans.size());
        assertTrue(ambiguous.message.startsWith("2 beans of type 'Greeting' match"), ambiguous.message);
        assertEquals("resolved", byName.status);
        assertEquals("french", byName.beans.get(0).name);
        assertEquals("resolved", store.status);
        assertEquals("mainStore", store.beans.get(0).name);
        assertEquals("missing", missing.status);
        assertEquals("No bean of type 'Audit' found: other.Outside is not in a scanned package", missing.message);
        assertEquals("optional", optional.status);
        assertEquals("unknown", feign.status);
        assertEquals("multiple", all.status);
        assertEquals(2, r.diagnostics.get("file:///app.App.java").size());
    }

    @Test
    void evaluatesProfilesWhenTheProjectSaysWhichAreActive() {
        Facts f = new Facts();
        f.scans.add(application("app.App", "app"));
        f.beans.add(component("app.App"));
        BeanDef dev = component("app.DevMail", "app.Mail");
        dev.profiles.add("dev | local");
        BeanDef prod = component("app.SmtpMail", "app.Mail");
        prod.profiles.add("!dev & !local");
        f.beans.add(dev);
        f.beans.add(prod);
        InjectionPoint mail = inject("app.App", "mail", "app.Mail");
        f.injections.add(mail);
        Map<String, Resolver.ProjectInput> projects = new LinkedHashMap<>();
        projects.put("app", project("app", f));
        // Not evaluated: both may be absent, so neither makes it ambiguous, and it resolves.
        Resolver.Result r = new Resolver(projects, ORACLE).resolve();
        assertEquals("resolved", mail.status);
        assertEquals(2, mail.beans.size());
        assertTrue(r.contexts.get("app").inactive.isEmpty());
        // dev: the dev bean alone, the other listed as inactive.
        projects.get("app").activeProfiles = java.util.Set.of("dev");
        r = new Resolver(projects, ORACLE).resolve();
        assertEquals("resolved", mail.status);
        assertEquals(List.of("devMail"), mail.beans.stream().map(b -> b.name).toList());
        Resolver.Context c = r.contexts.get("app");
        assertTrue(c.inactive.contains(prod) && c.active(dev));
        // default: the SMTP one.
        projects.get("app").activeProfiles = java.util.Set.of("default");
        new Resolver(projects, ORACLE).resolve();
        assertEquals(List.of("smtpMail"), mail.beans.stream().map(b -> b.name).toList());
        // Two that match the active profiles are ambiguous now, as Spring would find them.
        prod.profiles.set(0, "dev");
        projects.get("app").activeProfiles = java.util.Set.of("dev");
        new Resolver(projects, ORACLE).resolve();
        assertEquals("ambiguous", mail.status);
    }

    @Test
    void readsXmlAndQualifiers() {
        Facts f = new Facts();
        f.scans.add(application("app.App", "app"));
        f.beans.add(component("app.App"));
        f.importResources.put("app.App", new java.util.LinkedHashSet<>(List.of("classpath:beans.xml")));
        f.beans.add(component("lib.extra.Helper"));
        BeanDef fast = component("app.Fast", "app.Engine");
        fast.qualifiers.add("fast");
        f.beans.add(fast);
        f.beans.add(component("app.Slow", "app.Engine"));
        InjectionPoint rest = inject("app.App", "rest", "org.web.RestTemplate");
        InjectionPoint helper = inject("app.App", "helper", "lib.extra.Helper");
        InjectionPoint engine = inject("app.App", "engine", "app.Engine");
        engine.qualifiers.add("fast");
        InjectionPoint wrong = inject("app.App", "engine2", "app.Engine");
        wrong.qualifiers.add("turbo");
        f.injections.addAll(List.of(rest, helper, engine, wrong));
        Map<String, Resolver.ProjectInput> projects = new LinkedHashMap<>();
        projects.put("app", project("app", f));
        new Resolver(projects, ORACLE).resolve();
        assertEquals("resolved", rest.status);
        assertEquals("xml", rest.beans.get(0).kind);
        assertEquals("resolved", helper.status);
        assertEquals("fast", engine.beans.get(0).name);
        assertEquals("missing", wrong.status);
        assertEquals("No bean of type 'Engine' qualified 'turbo' found", wrong.message);
    }

    @Test
    void checksSharedCodeInEveryApplication() {
        // core is compiled into two services; each defines its own Notifier.
        Facts core = new Facts();
        core.beans.add(component("core.Billing"));
        InjectionPoint notifier = inject("core.Billing", "notifier", "core.Notifier");
        core.injections.add(notifier);
        Facts a = new Facts();
        a.scans.add(application("core.a.A", "core"));
        a.beans.add(component("core.a.A"));
        a.beans.add(component("core.a.Sms", "core.Notifier"));
        Facts b = new Facts();
        b.scans.add(application("core.b.B", "core"));
        b.beans.add(component("core.b.B"));
        b.beans.add(component("core.b.Mail", "core.Notifier"));
        b.beans.add(component("core.b.Push", "core.Notifier"));
        Map<String, Resolver.ProjectInput> projects = new LinkedHashMap<>();
        projects.put("core", project("core", core));
        projects.put("a", project("a", a, "core"));
        projects.put("b", project("b", b, "core"));
        new Resolver(projects, ORACLE).resolve();
        // Resolved in a, ambiguous in b: b's problem is reported.
        assertEquals("ambiguous", notifier.status);
        assertEquals(2, notifier.beans.size());
    }
}
