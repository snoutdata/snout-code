package com.snoutcode.engine.spring;

import com.google.gson.JsonObject;
import org.eclipse.jdt.core.dom.ASTNode;
import org.eclipse.jdt.core.dom.ASTVisitor;
import org.eclipse.jdt.core.dom.AbstractTypeDeclaration;
import org.eclipse.jdt.core.dom.Annotation;
import org.eclipse.jdt.core.dom.BooleanLiteral;
import org.eclipse.jdt.core.dom.CompilationUnit;
import org.eclipse.jdt.core.dom.Expression;
import org.eclipse.jdt.core.dom.FieldDeclaration;
import org.eclipse.jdt.core.dom.IBinding;
import org.eclipse.jdt.core.dom.IMethodBinding;
import org.eclipse.jdt.core.dom.ITypeBinding;
import org.eclipse.jdt.core.dom.IVariableBinding;
import org.eclipse.jdt.core.dom.ImportDeclaration;
import org.eclipse.jdt.core.dom.Javadoc;
import org.eclipse.jdt.core.dom.MarkerAnnotation;
import org.eclipse.jdt.core.dom.MemberValuePair;
import org.eclipse.jdt.core.dom.MethodDeclaration;
import org.eclipse.jdt.core.dom.NormalAnnotation;
import org.eclipse.jdt.core.dom.NumberLiteral;
import org.eclipse.jdt.core.dom.RecordDeclaration;
import org.eclipse.jdt.core.dom.SimpleName;
import org.eclipse.jdt.core.dom.SingleMemberAnnotation;
import org.eclipse.jdt.core.dom.SingleVariableDeclaration;
import org.eclipse.jdt.core.dom.StringLiteral;
import org.eclipse.jdt.core.dom.TagElement;
import org.eclipse.jdt.core.dom.TextElement;
import org.eclipse.jdt.core.dom.VariableDeclarationFragment;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Positions in a Java compilation unit, from its AST; members declared elsewhere go to {@link Locations}. */
final class SourcePositions implements TypeScanner.Positions {
    private final CompilationUnit ast;
    private final String uri;
    private final String source;
    private final Locations elsewhere;
    private final Map<String, ASTNode> nodes = new HashMap<>();

    SourcePositions(CompilationUnit ast, String uri, String source, Locations elsewhere) {
        this.ast = ast;
        this.uri = uri;
        this.source = source;
        this.elsewhere = elsewhere;
        ast.accept(new ASTVisitor() {
            @Override
            public void endVisit(org.eclipse.jdt.core.dom.TypeDeclaration node) {
                put(node);
            }

            @Override
            public void endVisit(org.eclipse.jdt.core.dom.EnumDeclaration node) {
                put(node);
            }

            @Override
            public void endVisit(RecordDeclaration node) {
                put(node);
            }

            private void put(AbstractTypeDeclaration node) {
                ITypeBinding b = node.resolveBinding();
                if (b != null) {
                    nodes.put(b.getKey(), node);
                }
            }

            @Override
            public boolean visit(VariableDeclarationFragment node) {
                if (node.getParent() instanceof FieldDeclaration) {
                    IVariableBinding b = node.resolveBinding();
                    if (b != null) {
                        nodes.put(b.getVariableDeclaration().getKey(), node);
                    }
                }
                return false;
            }

            @Override
            public boolean visit(MethodDeclaration node) {
                IMethodBinding b = node.resolveBinding();
                if (b != null) {
                    nodes.put(b.getMethodDeclaration().getKey(), node);
                }
                return true;
            }
        });
    }

    Loc range(ASTNode node) {
        int start = node.getStartPosition();
        int end = start + node.getLength();
        return new Loc(uri, ast.getLineNumber(start) - 1, ast.getColumnNumber(start), ast.getLineNumber(end) - 1, ast.getColumnNumber(end));
    }

    @Override
    public Loc type(ITypeBinding t) {
        ASTNode node = nodes.get(t.getKey());
        if (node instanceof AbstractTypeDeclaration d) {
            return range(d.getName());
        }
        return elsewhere.of(t);
    }

    @Override
    public Loc field(IVariableBinding f) {
        ASTNode node = nodes.get(f.getVariableDeclaration().getKey());
        if (node instanceof VariableDeclarationFragment v) {
            return range(v.getName());
        }
        return elsewhere.of(f);
    }

    @Override
    public Loc method(IMethodBinding m) {
        ASTNode node = nodes.get(m.getMethodDeclaration().getKey());
        if (node instanceof MethodDeclaration d) {
            return range(d.getName());
        }
        return elsewhere.of(m);
    }

    @Override
    public Loc param(IMethodBinding m, int index) {
        ASTNode p = parameter(m, index);
        if (p instanceof SingleVariableDeclaration v) {
            return range(v.getName());
        }
        ASTNode node = nodes.get(m.getMethodDeclaration().getKey());
        if (node instanceof MethodDeclaration d) {
            return range(d.getName());
        }
        ASTNode type = nodes.get(m.getDeclaringClass().getKey());
        if (type instanceof AbstractTypeDeclaration d) {
            return range(d.getName());
        }
        return elsewhere.parameter(m, index);
    }

    private ASTNode parameter(IMethodBinding m, int index) {
        ASTNode node = nodes.get(m.getMethodDeclaration().getKey());
        if (node instanceof MethodDeclaration d && index < d.parameters().size()) {
            return (ASTNode) d.parameters().get(index);
        }
        ASTNode type = nodes.get(m.getDeclaringClass().getKey());
        if (m.isConstructor() && type instanceof RecordDeclaration r && index < r.recordComponents().size()) {
            return (ASTNode) r.recordComponents().get(index);
        }
        return null;
    }

    /** The modifiers (annotations among them) of a field, a method, or one of its parameters. */
    @SuppressWarnings("unchecked")
    private List<Object> modifiers(IBinding element, int param) {
        if (element instanceof IVariableBinding f) {
            ASTNode node = nodes.get(f.getVariableDeclaration().getKey());
            if (node != null && node.getParent() instanceof FieldDeclaration fd) {
                return fd.modifiers();
            }
        } else if (element instanceof IMethodBinding m) {
            if (param >= 0) {
                ASTNode p = parameter(m, param);
                if (p instanceof SingleVariableDeclaration v) {
                    return v.modifiers();
                }
            } else {
                ASTNode node = nodes.get(m.getMethodDeclaration().getKey());
                if (node instanceof MethodDeclaration d) {
                    return d.modifiers();
                }
            }
        }
        return List.of();
    }

    private static Annotation annotation(List<Object> modifiers, String fqn) {
        for (Object o : modifiers) {
            if (o instanceof Annotation a) {
                ITypeBinding t = a.resolveTypeBinding();
                String name = t == null ? a.getTypeName().getFullyQualifiedName() : t.getQualifiedName();
                if (fqn.equals(name) || (t == null && fqn.endsWith("." + name))) {
                    return a;
                }
            }
        }
        return null;
    }

    @Override
    public Loc literal(IBinding element, int param, String annotation) {
        Annotation a = annotation(modifiers(element, param), annotation);
        Expression value = null;
        if (a instanceof SingleMemberAnnotation s) {
            value = s.getValue();
        } else if (a instanceof NormalAnnotation n) {
            for (Object o : n.values()) {
                MemberValuePair pair = (MemberValuePair) o;
                if ("value".equals(pair.getName().getIdentifier())) {
                    value = pair.getValue();
                }
            }
        }
        return value instanceof StringLiteral ? range(value) : null;
    }

    @Override
    public JsonObject fix(IBinding element, int param) {
        JsonObject fix = new JsonObject();
        int at;
        boolean inline;
        if (element instanceof IVariableBinding f) {
            ASTNode node = nodes.get(f.getVariableDeclaration().getKey());
            if (node == null || !(node.getParent() instanceof FieldDeclaration fd)) {
                return null;
            }
            at = fd.modifiers().isEmpty() ? fd.getType().getStartPosition() : ((ASTNode) fd.modifiers().get(0)).getStartPosition();
            inline = false;
        } else if (element instanceof IMethodBinding m && param >= 0) {
            ASTNode p = parameter(m, param);
            if (!(p instanceof SingleVariableDeclaration v)) {
                return null;
            }
            at = v.getStartPosition();
            inline = true;
        } else {
            return null;
        }
        fix.addProperty("qualifierLine", ast.getLineNumber(at) - 1);
        fix.addProperty("qualifierCharacter", ast.getColumnNumber(at));
        fix.addProperty("qualifierInline", inline);
        int lineStart = at - ast.getColumnNumber(at);
        fix.addProperty("indent", source.substring(Math.max(0, lineStart), at).replaceAll("\\S", " "));
        if (!imports(Annotations.QUALIFIER)) {
            fix.addProperty("importLine", importLine());
        }
        boolean constructor = element instanceof IMethodBinding m && m.isConstructor();
        Annotation autowired = constructor ? null : annotation(modifiers(element, -1), Annotations.AUTOWIRED);
        if (autowired instanceof MarkerAnnotation) {
            fix.add("autowired", range(autowired).range());
        }
        return fix;
    }

    private boolean imports(String fqn) {
        String pkg = Names.packageOf(fqn);
        for (Object o : ast.imports()) {
            ImportDeclaration i = (ImportDeclaration) o;
            String name = i.getName().getFullyQualifiedName();
            if (!i.isStatic() && (name.equals(fqn) || (i.isOnDemand() && name.equals(pkg)))) {
                return true;
            }
        }
        return false;
    }

    private int importLine() {
        List<?> imports = ast.imports();
        if (!imports.isEmpty()) {
            ASTNode last = (ASTNode) imports.get(imports.size() - 1);
            return ast.getLineNumber(last.getStartPosition() + last.getLength()); // the line after it, 0-based
        }
        if (ast.getPackage() != null) {
            ASTNode p = ast.getPackage();
            return ast.getLineNumber(p.getStartPosition() + p.getLength());
        }
        return 0;
    }

    @Override
    public String javadoc(IBinding member) {
        if (!(member instanceof IVariableBinding f)) {
            return null;
        }
        ASTNode node = nodes.get(f.getVariableDeclaration().getKey());
        if (node == null || !(node.getParent() instanceof FieldDeclaration fd)) {
            return null;
        }
        Javadoc doc = fd.getJavadoc();
        if (doc == null) {
            return null;
        }
        StringBuilder text = new StringBuilder();
        for (Object t : doc.tags()) {
            TagElement tag = (TagElement) t;
            if (tag.getTagName() != null) {
                break;
            }
            for (Object f2 : tag.fragments()) {
                if (f2 instanceof TextElement te) {
                    text.append(te.getText().strip()).append(' ');
                } else if (f2 instanceof TagElement inner) {
                    for (Object g : inner.fragments()) {
                        if (g instanceof TextElement te2) {
                            text.append(te2.getText().strip()).append(' ');
                        } else if (g instanceof ASTNode n) {
                            text.append(n.toString().strip()).append(' ');
                        }
                    }
                } else if (f2 instanceof ASTNode n) {
                    text.append(n.toString().strip()).append(' ');
                }
            }
        }
        String s = text.toString().strip();
        return s.isEmpty() ? null : s;
    }

    @Override
    public String initializer(IVariableBinding f) {
        ASTNode node = nodes.get(f.getVariableDeclaration().getKey());
        if (!(node instanceof VariableDeclarationFragment v) || v.getInitializer() == null) {
            return null;
        }
        Expression e = v.getInitializer();
        if (e instanceof StringLiteral s) {
            return s.getLiteralValue();
        }
        if (e instanceof NumberLiteral n) {
            return n.getToken().replaceAll("[lLdDfF]$", "");
        }
        if (e instanceof BooleanLiteral b) {
            return String.valueOf(b.booleanValue());
        }
        if (e instanceof SimpleName || e.getClass().getSimpleName().equals("QualifiedName")) {
            IBinding b = e instanceof SimpleName sn ? sn.resolveBinding() : ((org.eclipse.jdt.core.dom.QualifiedName) e).resolveBinding();
            if (b instanceof IVariableBinding vb && vb.isEnumConstant()) {
                return vb.getName().toLowerCase(java.util.Locale.ROOT).replace('_', '-');
            }
        }
        return null;
    }
}
