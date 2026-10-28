package com.snoutcode.engine.spring;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class XmlBeansTest {
    @Test
    void readsBeansScansAndPlaceholders() {
        String xml = """
            <beans xmlns="http://www.springframework.org/schema/beans"
                   xmlns:context="http://www.springframework.org/schema/context">
              <context:component-scan base-package="a.b, c.d" />
              <bean id="restTemplate" name="rest,client" class="org.springframework.web.client.RestTemplate">
                <property name="x"><bean class="inner.Bean"/></property>
              </bean>
              <bean class="org.mybatis.spring.mapper.MapperScannerConfigurer">
                <property name="basePackage" value="e.f.mapper"/>
              </bean>
              <bean id="ds" class="x.DataSource"><property name="url" value="${jdbc.url}"/></bean>
              <import resource="classpath:more.xml"/>
            </beans>""";
        Facts f = XmlBeans.parse(xml, "file:///app.xml");
        assertEquals(3, f.beans.size());
        assertEquals("restTemplate", f.beans.get(0).name);
        assertEquals(java.util.List.of("rest", "client"), f.beans.get(0).aliases);
        assertEquals(3, f.beans.get(0).loc.line);
        assertEquals(2, f.scans.size());
        assertEquals(java.util.List.of("a.b", "c.d"), f.scans.get(0).packages);
        assertEquals("mapperScan", f.scans.get(1).kind);
        assertEquals("jdbc.url", f.keyUses.get(0).key);
        assertEquals("[classpath:more.xml]", f.importResources.get("file:///app.xml").toString());
    }

    @Test
    void readsTheSourceFileOfAClass() throws Exception {
        try (var in = XmlBeansTest.class.getResourceAsStream("XmlBeansTest.class")) {
            assertEquals("XmlBeansTest.java", ClassFiles.sourceFile(in.readAllBytes()));
        }
    }

    @Test
    void findsScalaDeclarations() {
        ScalaLocator s = new ScalaLocator("file:///X.scala", """
            package x
            @Controller
            class OrderController @Autowired()(orders: OrderService) extends Base {
              @Autowired
              val service: OrderTypeService = null
              @RequestMapping(Array("/list"))
              def list(): String = ""
            }
            """);
        assertEquals(2, s.type("OrderController").line);
        assertEquals(4, s.member("OrderController", "service", false).line);
        assertEquals(6, s.member("OrderController", "list", true).line);
        assertEquals(2, s.parameter("OrderController", "orders").line);
    }
}
