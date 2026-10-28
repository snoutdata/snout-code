package com.snoutcode.engine.spring;

import com.snoutcode.engine.spring.Facts.BeanDef;
import com.snoutcode.engine.spring.Facts.KeyUse;
import com.snoutcode.engine.spring.Facts.ScanSpec;
import org.xml.sax.Attributes;
import org.xml.sax.InputSource;
import org.xml.sax.Locator;
import org.xml.sax.helpers.DefaultHandler;

import javax.xml.parsers.SAXParser;
import javax.xml.parsers.SAXParserFactory;
import java.io.StringReader;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * Reads a Spring XML bean file: {@code <bean>} definitions and aliases, component scans,
 * MyBatis mapper scans, {@code <import>}s of other files, and the {@code ${...}} property keys
 * it uses.
 */
final class XmlBeans {
    private XmlBeans() {
    }

    /** The facts of one XML file; {@code location} is how it was imported, for its own imports. */
    static Facts parse(String text, String uri) {
        Facts facts = new Facts();
        LineIndex lines = new LineIndex(text);
        try {
            SAXParserFactory factory = SAXParserFactory.newInstance();
            factory.setNamespaceAware(true);
            factory.setValidating(false);
            factory.setFeature("http://apache.org/xml/features/nonvalidating/load-external-dtd", false);
            factory.setFeature("http://xml.org/sax/features/external-general-entities", false);
            factory.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
            SAXParser parser = factory.newSAXParser();
            parser.parse(new InputSource(new StringReader(text)), new Handler(facts, text, lines, uri));
        } catch (Exception e) {
            // A broken file yields what was read before the error.
        }
        return facts;
    }

    private static final class Handler extends DefaultHandler {
        private final Facts facts;
        private final String text;
        private final LineIndex lines;
        private final String uri;
        private Locator locator;
        private final Deque<BeanDef> beans = new ArrayDeque<>();
        private final Deque<String> elements = new ArrayDeque<>();
        private int anonymous;

        Handler(Facts facts, String text, LineIndex lines, String uri) {
            this.facts = facts;
            this.text = text;
            this.lines = lines;
            this.uri = uri;
        }

        @Override
        public void setDocumentLocator(Locator locator) {
            this.locator = locator;
        }

        @Override
        public void startElement(String ns, String local, String qName, Attributes atts) {
            elements.push(qName);
            for (int i = 0; i < atts.getLength(); i++) {
                keys(atts.getValue(i));
            }
            String namespace = ns == null ? "" : ns;
            if ("bean".equals(local) && namespace.endsWith("/schema/beans")) {
                BeanDef b = new BeanDef();
                String cls = atts.getValue("class");
                String id = atts.getValue("id");
                String names = atts.getValue("name");
                Set<String> all = new LinkedHashSet<>();
                if (id != null && !id.isBlank()) {
                    all.add(id.trim());
                }
                if (names != null) {
                    for (String n : names.split("[,; ]+")) {
                        if (!n.isBlank()) {
                            all.add(n.trim());
                        }
                    }
                }
                b.kind = "xml";
                b.language = "xml";
                b.type = cls == null ? null : cls.trim();
                b.declaringType = b.type;
                if (b.type != null) {
                    b.assignable.add(b.type); // the bundle adds its supertypes
                }
                b.packageName = b.type == null ? null : Names.packageOf(b.type);
                if (all.isEmpty()) {
                    b.name = (b.type == null ? "bean" : b.type) + "#" + anonymous++;
                } else {
                    var it = all.iterator();
                    b.name = it.next();
                    it.forEachRemaining(b.aliases::add);
                }
                b.primary = "true".equals(atts.getValue("primary"));
                b.autowireCandidate = !"false".equals(atts.getValue("autowire-candidate"));
                if (atts.getValue("factory-bean") != null || atts.getValue("factory-method") != null) {
                    b.method = atts.getValue("factory-method");
                    b.declaringType = atts.getValue("factory-bean") == null ? b.type : null;
                    b.type = null; // unknown without the factory method's return type
                    b.assignable.clear();
                }
                if (atts.getValue("abstract") != null && "true".equals(atts.getValue("abstract"))) {
                    b.isAbstract = true;
                }
                b.loc = here(qName);
                // A nested bean is an inner bean, not a definition of the context.
                if (beans.isEmpty() && !b.isAbstract) {
                    facts.beans.add(b);
                }
                beans.push(b);
            } else if ("alias".equals(local) && namespace.endsWith("/schema/beans")) {
                String name = atts.getValue("name");
                String alias = atts.getValue("alias");
                for (BeanDef b : facts.beans) {
                    if (b.name.equals(name) && alias != null) {
                        b.aliases.add(alias);
                    }
                }
            } else if ("import".equals(local) && namespace.endsWith("/schema/beans")) {
                String resource = atts.getValue("resource");
                if (resource != null) {
                    facts.importResources.computeIfAbsent(uri, k -> new LinkedHashSet<>()).add(resource.trim());
                }
            } else if ("component-scan".equals(local) && namespace.endsWith("/schema/context")) {
                scan("componentScan", atts.getValue("base-package"), qName);
            } else if ("scan".equals(local) && namespace.contains("mybatis")) {
                scan("mapperScan", atts.getValue("base-package"), qName);
            } else if ("property".equals(local) && !beans.isEmpty()) {
                BeanDef owner = beans.peek();
                if ("basePackage".equals(atts.getValue("name")) && owner.type != null && owner.type.endsWith("MapperScannerConfigurer")) {
                    scan("mapperScan", atts.getValue("value"), qName);
                }
            }
        }

        @Override
        public void endElement(String ns, String local, String qName) {
            elements.pop();
            if ("bean".equals(local) && ns != null && ns.endsWith("/schema/beans") && !beans.isEmpty()) {
                beans.pop();
            }
        }

        private void scan(String kind, String packages, String element) {
            if (packages == null) {
                return;
            }
            ScanSpec s = new ScanSpec();
            s.kind = kind;
            s.declaringType = uri;
            for (String p : packages.split("[,; \t\n]+")) {
                if (!p.isBlank()) {
                    s.packages.add(p.trim());
                }
            }
            s.loc = here(element);
            facts.scans.add(s);
        }

        private void keys(String value) {
            for (Placeholders.Key k : Placeholders.keys(value)) {
                KeyUse use = new KeyUse();
                use.key = k.key();
                use.defaultValue = k.defaultValue();
                use.loc = here(elements.peek());
                facts.keyUses.add(use);
            }
        }

        /** The start tag that ends where the parser is: SAX reports the position after it. */
        private Loc here(String qName) {
            int end = lines.offset(locator.getLineNumber() - 1, Math.max(0, locator.getColumnNumber() - 1));
            int start = text.lastIndexOf("<" + qName, Math.max(0, end - 1));
            if (start < 0) {
                return Loc.of(uri, lines, end, 0);
            }
            return Loc.of(uri, lines, start + 1, qName.length());
        }
    }
}
