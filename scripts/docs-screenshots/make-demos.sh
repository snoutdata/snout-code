#!/bin/bash
# The demo projects of the user guide's screenshots, made under $1: shop (a Spring Boot web
# app, one test failing on purpose), ledger (a build of three modules with a version catalog),
# greeter (a small Java app) and pantry (a Maven build: a library and a Spring Boot app under a
# parent that imports Spring Boot's BOM, with profiles and one test failing on purpose). Each is
# a Git repository with a little history. Run by scripts/docs-screenshots.sh.
set -euo pipefail
OUT="$1"
# The Gradle wrapper comes from the engine's build.
WRAPPER="$(cd "$(dirname "$0")/../../engine" && pwd)"
rm -rf "$OUT/shop" "$OUT/ledger" "$OUT/greeter" "$OUT/pantry"
mkdir -p "$OUT"

wrapper() {
  mkdir -p "$1/gradle/wrapper"
  cp "$WRAPPER/gradlew" "$1/"
  cp "$WRAPPER/gradle/wrapper/gradle-wrapper.jar" "$WRAPPER/gradle/wrapper/gradle-wrapper.properties" "$1/gradle/wrapper/"
  printf '.gradle/\nbuild/\n*.class\n.idea/\n' >"$1/.gitignore"
}

# Commit as one of two demo authors, at a fixed time so the log reads like a week of work.
T=1792000000
commit() {
  local dir="$1" who="$2" msg="$3"
  T=$((T + 28000 + RANDOM % 40000))
  local name email
  case "$who" in
    alex) name="Alex Chen"; email="alex@example.com" ;;
    sam) name="Sam Rivera"; email="sam@example.com" ;;
  esac
  git -C "$dir" add -A
  GIT_AUTHOR_NAME="$name" GIT_AUTHOR_EMAIL="$email" GIT_COMMITTER_NAME="$name" GIT_COMMITTER_EMAIL="$email" \
    GIT_AUTHOR_DATE="@$T +0000" GIT_COMMITTER_DATE="@$T +0000" \
    git -C "$dir" -c commit.gpgsign=false commit -qm "$msg"
}
merge() {
  local dir="$1" branch="$2" msg="$3"
  T=$((T + 3600))
  GIT_AUTHOR_NAME="Alex Chen" GIT_AUTHOR_EMAIL="alex@example.com" GIT_COMMITTER_NAME="Alex Chen" \
    GIT_COMMITTER_EMAIL="alex@example.com" GIT_AUTHOR_DATE="@$T +0000" GIT_COMMITTER_DATE="@$T +0000" \
    git -C "$dir" -c commit.gpgsign=false merge -q --no-ff -m "$msg" "$branch"
}
init() {
  git -C "$1" init -q -b main
  git -C "$1" config user.name "Alex Chen"
  git -C "$1" config user.email "alex@example.com"
  git -C "$1" config commit.gpgsign false
  git -C "$1" remote add origin "git@github.com:example/$(basename "$1").git"
}

############################################################ shop
S="$OUT/shop"
P="$S/src/main/java/com/example/shop"
TP="$S/src/test/java/com/example/shop"
mkdir -p "$P"/{web,service,repository,model} "$S/src/main/resources" "$TP"/service
wrapper "$S"
init "$S"
cat >"$S/settings.gradle.kts" <<'EOF'
rootProject.name = "shop"
EOF
cat >"$S/build.gradle.kts" <<'EOF'
plugins {
    java
    id("org.springframework.boot") version "3.1.1"
    id("io.spring.dependency-management") version "1.1.0"
}

group = "com.example"
version = "0.3.0"

java {
    sourceCompatibility = JavaVersion.VERSION_21
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.springframework.boot:spring-boot-starter-web")
    testImplementation("org.springframework.boot:spring-boot-starter-test")
}

tasks.test {
    useJUnitPlatform()
}
EOF
cat >"$P/ShopApplication.java" <<'EOF'
package com.example.shop;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

@SpringBootApplication
public class ShopApplication {

    public static void main(String[] args) {
        SpringApplication.run(ShopApplication.class, args);
    }
}
EOF
cat >"$P/model/Product.java" <<'EOF'
package com.example.shop.model;

import java.math.BigDecimal;

/** Something the shop sells. */
public record Product(long id, String name, BigDecimal price, int stock) {

    public boolean inStock() {
        return stock > 0;
    }
}
EOF
cat >"$S/README.md" <<'EOF'
# shop

A small Spring Boot web shop: products, orders and a price calculator.

    ./gradlew bootRun
EOF
cat >"$S/src/main/resources/application.yml" <<'EOF'
spring:
  application:
    name: shop

server:
  port: 8080
EOF
cat >"$P/repository/ProductRepository.java" <<'EOF'
package com.example.shop.repository;

import com.example.shop.model.Product;
import java.math.BigDecimal;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import org.springframework.stereotype.Repository;

/** The products, kept in memory. */
@Repository
public class ProductRepository {

    private final Map<Long, Product> products = new ConcurrentHashMap<>();

    public ProductRepository() {
        save(new Product(1, "Espresso beans", new BigDecimal("12.50"), 40));
        save(new Product(2, "Pour-over kettle", new BigDecimal("39.00"), 8));
        save(new Product(3, "Ceramic mug", new BigDecimal("9.75"), 0));
    }

    public List<Product> findAll() {
        return List.copyOf(products.values());
    }

    public Optional<Product> findById(long id) {
        return Optional.ofNullable(products.get(id));
    }

    public Product save(Product product) {
        products.put(product.id(), product);
        return product;
    }
}
EOF
cat >"$P/service/ProductService.java" <<'EOF'
package com.example.shop.service;

import com.example.shop.model.Product;
import com.example.shop.repository.ProductRepository;
import java.util.List;
import org.springframework.stereotype.Service;

@Service
public class ProductService {

    private final ProductRepository products;

    public ProductService(ProductRepository products) {
        this.products = products;
    }

    public List<Product> available() {
        return products.findAll().stream().filter(Product::inStock).toList();
    }

    public Product get(long id) {
        return products.findById(id).orElseThrow(() -> new ProductNotFound(id));
    }
}
EOF
cat >"$P/service/ProductNotFound.java" <<'EOF'
package com.example.shop.service;

public class ProductNotFound extends RuntimeException {

    public ProductNotFound(long id) {
        super("No product " + id);
    }
}
EOF
cat >"$P/web/ProductController.java" <<'EOF'
package com.example.shop.web;

import com.example.shop.model.Product;
import com.example.shop.service.ProductService;
import java.util.List;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/products")
public class ProductController {

    private final ProductService productService;

    public ProductController(ProductService productService) {
        this.productService = productService;
    }

    @GetMapping
    public List<Product> list() {
        return productService.available();
    }

    @GetMapping("/{id}")
    public Product get(@PathVariable long id) {
        return productService.get(id);
    }
}
EOF
cat >"$TP/service/ProductServiceTest.java" <<'EOF'
package com.example.shop.service;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import com.example.shop.repository.ProductRepository;
import org.junit.jupiter.api.Test;

class ProductServiceTest {

    private final ProductService service = new ProductService(new ProductRepository());

    @Test
    void listsOnlyWhatIsInStock() {
        assertEquals(2, service.available().size());
    }

    @Test
    void findsAProductById() {
        assertEquals("Espresso beans", service.get(1).name());
    }

    @Test
    void failsForAnUnknownProduct() {
        assertThrows(ProductNotFound.class, () -> service.get(99));
    }
}
EOF
commit "$S" alex "Spring Boot shop with a products API"

cat >"$P/model/Order.java" <<'EOF'
package com.example.shop.model;

import java.math.BigDecimal;
import java.util.List;

/** An order: the lines a customer bought, and what they paid. */
public record Order(long id, String customer, List<Line> lines, BigDecimal total) {

    public record Line(long productId, int quantity) {}
}
EOF
cat >"$P/repository/OrderRepository.java" <<'EOF'
package com.example.shop.repository;

import com.example.shop.model.Order;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicLong;
import org.springframework.stereotype.Repository;

@Repository
public class OrderRepository {

    private final List<Order> orders = new ArrayList<>();
    private final AtomicLong ids = new AtomicLong(1000);

    public long nextId() {
        return ids.incrementAndGet();
    }

    public synchronized Order save(Order order) {
        orders.add(order);
        return order;
    }

    public synchronized List<Order> findByCustomer(String customer) {
        return orders.stream().filter(o -> o.customer().equals(customer)).toList();
    }
}
EOF
cat >"$P/service/PriceCalculator.java" <<'EOF'
package com.example.shop.service;

import java.math.BigDecimal;
import java.math.RoundingMode;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

/** Prices with tax, and the discount for large orders. */
@Component
public class PriceCalculator {

    private final BigDecimal taxRate;

    public PriceCalculator(@Value("${shop.tax-rate:0.20}") BigDecimal taxRate) {
        this.taxRate = taxRate;
    }

    /**
     * The price with tax added, rounded half up to the cent.
     *
     * @param net the price before tax
     * @return the price the customer pays
     */
    public BigDecimal withTax(BigDecimal net) {
        return net.add(net.multiply(taxRate)).setScale(2, RoundingMode.HALF_UP);
    }

    public BigDecimal discount(BigDecimal total, int items) {
        if (items >= 3) {
            return total.multiply(new BigDecimal("0.10")).setScale(2, RoundingMode.HALF_UP);
        }
        return BigDecimal.ZERO;
    }
}
EOF
cat >"$P/service/OrderService.java" <<'EOF'
package com.example.shop.service;

import com.example.shop.model.Order;
import com.example.shop.model.Product;
import com.example.shop.repository.OrderRepository;
import java.math.BigDecimal;
import java.util.List;
import org.springframework.stereotype.Service;

@Service
public class OrderService {

    private final ProductService productService;
    private final OrderRepository orders;
    private final PriceCalculator prices;

    public OrderService(ProductService productService, OrderRepository orders, PriceCalculator prices) {
        this.productService = productService;
        this.orders = orders;
        this.prices = prices;
    }

    public Order place(String customer, List<Order.Line> lines) {
        BigDecimal net = BigDecimal.ZERO;
        int items = 0;
        for (Order.Line line : lines) {
            Product product = productService.get(line.productId());
            net = net.add(product.price().multiply(BigDecimal.valueOf(line.quantity())));
            items += line.quantity();
        }
        BigDecimal total = prices.withTax(net.subtract(prices.discount(net, items)));
        return orders.save(new Order(orders.nextId(), customer, lines, total));
    }

    public List<Order> history(String customer) {
        return orders.findByCustomer(customer);
    }
}
EOF
cat >"$P/web/OrderController.java" <<'EOF'
package com.example.shop.web;

import com.example.shop.model.Order;
import com.example.shop.service.OrderService;
import java.util.List;
import org.springframework.http.HttpStatus;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/orders")
public class OrderController {

    private final OrderService orderService;

    public OrderController(OrderService orderService) {
        this.orderService = orderService;
    }

    @GetMapping
    public List<Order> history(@RequestParam String customer) {
        return orderService.history(customer);
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    public Order place(@RequestBody PlaceOrder request) {
        return orderService.place(request.customer(), request.lines());
    }

    public record PlaceOrder(String customer, List<Order.Line> lines) {}
}
EOF
commit "$S" alex "Orders: place an order and list a customer's history"

git -C "$S" switch -q -c feature/discounts
cat >"$TP/service/PriceCalculatorTest.java" <<'EOF'
package com.example.shop.service;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.math.BigDecimal;
import org.junit.jupiter.api.Test;

class PriceCalculatorTest {

    private final PriceCalculator prices = new PriceCalculator(new BigDecimal("0.20"));

    @Test
    void addsTheTax() {
        assertEquals(new BigDecimal("12.00"), prices.withTax(new BigDecimal("10.00")));
    }

    @Test
    void roundsHalfUp() {
        assertEquals(new BigDecimal("1.25"), prices.withTax(new BigDecimal("1.04")));
    }

    @Test
    void noDiscountForSmallOrders() {
        assertEquals(BigDecimal.ZERO, prices.discount(new BigDecimal("50.00"), 2));
    }

    @Test
    void tenPercentOffFromThreeItems() {
        assertEquals(new BigDecimal("5.00"), prices.discount(new BigDecimal("50.00"), 3));
    }
}
EOF
commit "$S" sam "Price calculator tests"
cat >"$TP/service/OrderServiceTest.java" <<'EOF'
package com.example.shop.service;

import static org.junit.jupiter.api.Assertions.assertEquals;

import com.example.shop.model.Order;
import com.example.shop.repository.OrderRepository;
import com.example.shop.repository.ProductRepository;
import java.math.BigDecimal;
import java.util.List;
import org.junit.jupiter.api.Test;

class OrderServiceTest {

    private final OrderService service = new OrderService(
            new ProductService(new ProductRepository()), new OrderRepository(), new PriceCalculator(new BigDecimal("0.20")));

    @Test
    void totalsAnOrderWithTax() {
        Order order = service.place("ada", List.of(new Order.Line(1, 2)));
        assertEquals(new BigDecimal("30.00"), order.total());
    }

    @Test
    void appliesTheDiscountFromThreeItems() {
        Order order = service.place("ada", List.of(new Order.Line(1, 2), new Order.Line(2, 1)));
        // 2 x 12.50 + 39.00 = 64.00, less 10% off
        assertEquals(new BigDecimal("57.60"), order.total());
    }

    @Test
    void remembersTheCustomersOrders() {
        service.place("grace", List.of(new Order.Line(2, 1)));
        assertEquals(1, service.history("grace").size());
    }
}
EOF
commit "$S" sam "Order totals and discounts are tested"
git -C "$S" switch -q main
cat >"$S/src/main/resources/application.yml" <<'EOF'
spring:
  application:
    name: shop
  jackson:
    default-property-inclusion: non_null

server:
  port: 8080

shop:
  tax-rate: 0.20
EOF
commit "$S" alex "Tax rate in application.yml"
merge "$S" feature/discounts "Merge branch 'feature/discounts'"
git -C "$S" switch -q -c feature/reviews
cat >"$P/model/Review.java" <<'EOF'
package com.example.shop.model;

public record Review(long productId, String author, int stars, String text) {}
EOF
commit "$S" sam "Product reviews: the model"
git -C "$S" switch -q main
git -C "$S" branch fix/rounding
git -C "$S" tag -a v0.2.0 -m "Orders and discounts" HEAD~1
cat >>"$S/README.md" <<'EOF'

## API

- `GET /api/products`: the products in stock
- `POST /api/orders`: place an order
EOF
commit "$S" alex "README: the API"
# Pushed up to the merge: one commit to push.
git -C "$S" update-ref refs/remotes/origin/main HEAD~1
git -C "$S" update-ref refs/remotes/origin/feature/discounts feature/discounts
git -C "$S" branch -q -u origin/main main
# Work in progress, for the Git view.
perl -0pi -e 's/        if \(items >= 3\) \{/        \/\/ Ten percent off from three items.\n        if (items >= 3) {/' "$P/service/PriceCalculator.java"
perl -0pi -e 's/        return BigDecimal.ZERO;\n    \}\n\}\n/        return BigDecimal.ZERO;\n    }\n\n    public BigDecimal shipping(BigDecimal total) {\n        return total.compareTo(FREE_SHIPPING_FROM) >= 0 ? BigDecimal.ZERO : new BigDecimal("4.90");\n    }\n}\n/' "$P/service/PriceCalculator.java"
perl -0pi -e 's/    private final BigDecimal taxRate;\n/    private static final BigDecimal FREE_SHIPPING_FROM = new BigDecimal("50.00");\n\n    private final BigDecimal taxRate;\n/' "$P/service/PriceCalculator.java"
perl -0pi -e 's/    public List<Order> history\(String customer\) \{/    public int count() {\n        return orders.findByCustomer("").size();\n    }\n\n    public List<Order> history(String customer) {/' "$P/service/OrderService.java"
cat >"$P/web/ProductNotFoundAdvice.java" <<'EOF'
package com.example.shop.web;

import com.example.shop.service.ProductNotFound;
import org.springframework.http.HttpStatus;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestControllerAdvice;

@RestControllerAdvice
class ProductNotFoundAdvice {

    @ExceptionHandler(ProductNotFound.class)
    @ResponseStatus(HttpStatus.NOT_FOUND)
    String notFound(ProductNotFound e) {
        return e.getMessage();
    }
}
EOF
git -C "$S" add "$P/web/ProductNotFoundAdvice.java"

############################################################ ledger
L="$OUT/ledger"
mkdir -p "$L/gradle" "$L"/core/src/main/java/com/example/ledger/core "$L"/core/src/test/java/com/example/ledger/core \
  "$L"/api/src/main/java/com/example/ledger/api "$L"/app/src/main/java/com/example/ledger/app
wrapper "$L"
init "$L"
cat >"$L/settings.gradle.kts" <<'EOF'
rootProject.name = "ledger"

include("core", "api", "app")
EOF
cat >"$L/gradle/libs.versions.toml" <<'EOF'
[versions]
guava = "31.1-jre"
junit = "5.9.3"

[libraries]
guava = { module = "com.google.guava:guava", version.ref = "guava" }
junit-bom = { module = "org.junit:junit-bom", version.ref = "junit" }
junit-jupiter = { module = "org.junit.jupiter:junit-jupiter" }
EOF
cat >"$L/build.gradle.kts" <<'EOF'
subprojects {
    apply(plugin = "java")

    repositories {
        mavenCentral()
    }

    extensions.configure<JavaPluginExtension> {
        sourceCompatibility = JavaVersion.VERSION_21
    }

    tasks.withType<Test> {
        useJUnitPlatform()
    }
}
EOF
cat >"$L/core/build.gradle.kts" <<'EOF'
dependencies {
    "implementation"(libs.guava)
    "testImplementation"(platform(libs.junit.bom))
    "testImplementation"(libs.junit.jupiter)
    "testRuntimeOnly"("org.junit.platform:junit-platform-launcher")
}
EOF
cat >"$L/api/build.gradle.kts" <<'EOF'
dependencies {
    "implementation"(project(":core"))
}
EOF
cat >"$L/app/build.gradle.kts" <<'EOF'
plugins {
    application
}

dependencies {
    implementation(project(":core"))
    implementation(project(":api"))
}

application {
    mainClass = "com.example.ledger.app.LedgerApp"
}
EOF
cat >"$L/core/src/main/java/com/example/ledger/core/Entry.java" <<'EOF'
package com.example.ledger.core;

import java.math.BigDecimal;
import java.time.LocalDate;

/** One line of the ledger: money in (positive) or out (negative). */
public record Entry(LocalDate date, String account, String memo, BigDecimal amount) {}
EOF
cat >"$L/core/src/main/java/com/example/ledger/core/Ledger.java" <<'EOF'
package com.example.ledger.core;

import com.google.common.collect.ImmutableList;
import java.math.BigDecimal;
import java.util.ArrayList;
import java.util.List;

public class Ledger {

    private final List<Entry> entries = new ArrayList<>();

    public void record(Entry entry) {
        entries.add(entry);
    }

    public BigDecimal balance(String account) {
        return entries.stream()
                .filter(e -> e.account().equals(account))
                .map(Entry::amount)
                .reduce(BigDecimal.ZERO, BigDecimal::add);
    }

    public ImmutableList<Entry> entries() {
        return ImmutableList.copyOf(entries);
    }
}
EOF
cat >"$L/core/src/test/java/com/example/ledger/core/LedgerTest.java" <<'EOF'
package com.example.ledger.core;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.math.BigDecimal;
import java.time.LocalDate;
import org.junit.jupiter.api.Test;

class LedgerTest {

    @Test
    void balanceSumsAnAccount() {
        Ledger ledger = new Ledger();
        ledger.record(new Entry(LocalDate.of(2026, 1, 5), "cash", "Opening", new BigDecimal("100.00")));
        ledger.record(new Entry(LocalDate.of(2026, 1, 9), "cash", "Coffee", new BigDecimal("-4.20")));
        assertEquals(new BigDecimal("95.80"), ledger.balance("cash"));
    }
}
EOF
cat >"$L/api/src/main/java/com/example/ledger/api/Statement.java" <<'EOF'
package com.example.ledger.api;

import com.example.ledger.core.Ledger;
import java.math.BigDecimal;

/** A statement of one account, for clients of the ledger. */
public record Statement(String account, BigDecimal balance) {

    public static Statement of(Ledger ledger, String account) {
        return new Statement(account, ledger.balance(account));
    }
}
EOF
cat >"$L/app/src/main/java/com/example/ledger/app/LedgerApp.java" <<'EOF'
package com.example.ledger.app;

import com.example.ledger.api.Statement;
import com.example.ledger.core.Entry;
import com.example.ledger.core.Ledger;
import java.math.BigDecimal;
import java.time.LocalDate;

public class LedgerApp {

    public static void main(String[] args) {
        Ledger ledger = new Ledger();
        ledger.record(new Entry(LocalDate.now(), "cash", "Opening", new BigDecimal("250.00")));
        ledger.record(new Entry(LocalDate.now(), "cash", "Groceries", new BigDecimal("-61.40")));
        System.out.println(Statement.of(ledger, "cash"));
    }
}
EOF
commit "$L" sam "Ledger: core, api and app modules"
git -C "$L" switch -q -c feature/csv-import
echo "// CSV import comes here." >"$L/core/src/main/java/com/example/ledger/core/CsvImport.java"
commit "$L" sam "Start the CSV import"
git -C "$L" switch -q main
printf '# ledger\n\nA double-entry ledger in three modules: core, api and app.\n' >"$L/README.md"
commit "$L" alex "README"

############################################################ greeter
G="$OUT/greeter"
mkdir -p "$G/src/main/java/demo" "$G/src/test/java/demo"
wrapper "$G"
init "$G"
cat >"$G/settings.gradle.kts" <<'EOF'
rootProject.name = "greeter"
EOF
cat >"$G/build.gradle.kts" <<'EOF'
plugins {
    application
}

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(platform("org.junit:junit-bom:5.9.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

application {
    mainClass = "demo.Main"
}

tasks.test {
    useJUnitPlatform()
}
EOF
cat >"$G/src/main/java/demo/Greeter.java" <<'EOF'
package demo;

import java.util.List;

/** Says hello. */
public class Greeter {
    private final String name;

    public Greeter(String name) {
        this.name = name;
    }

    // The greeting.
    public String greet() {
        return "Hello, " + name + "!";
    }

    public List<String> greetAll(List<String> names) {
        return names.stream().map(n -> "Hello, " + n).toList();
    }
}
EOF
cat >"$G/src/main/java/demo/Main.java" <<'EOF'
package demo;

import java.util.List;

public final class Main {
    public static void main(String[] args) {
        List<String> names = List.of("ada", "grace", "linus");
        int total = 0;
        for (int i = 1; i <= 3; i++) {
            total = add(total, i);
        }
        Greeter greeter = new Greeter(names.get(0));
        System.out.println(greeter.greet() + " total " + total);
    }

    static int add(int a, int b) {
        return a + b;
    }
}
EOF
cat >"$G/src/test/java/demo/GreeterTest.java" <<'EOF'
package demo;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class GreeterTest {

    @Test
    void greetsByName() {
        assertEquals("Hello, ada!", new Greeter("ada").greet());
    }
}
EOF
commit "$G" alex "A greeter"
perl -pi -e 's/\/\/ The greeting\./\/\/ The greeting, with the name./' "$G/src/main/java/demo/Greeter.java"
commit "$G" alex "Say what the greeting is"
############################################################ pantry
# A Maven reactor: a parent POM that imports Spring Boot's BOM, a library (stock) and a Spring
# Boot application (web) that uses it, a dev profile on by default and a prod one off. No
# wrapper: SnoutCode runs the Maven it downloads.
M="$OUT/pantry"
MS="$M/stock/src/main/java/com/example/pantry/stock"
MT="$M/stock/src/test/java/com/example/pantry/stock"
MW="$M/web/src/main/java/com/example/pantry/web"
MWT="$M/web/src/test/java/com/example/pantry/web"
mkdir -p "$MS" "$MT" "$MW" "$MWT" "$M/web/src/main/resources"
init "$M"
printf 'target/\n*.class\n.idea/\n' >"$M/.gitignore"
cat >"$M/pom.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>

  <groupId>com.example</groupId>
  <artifactId>pantry</artifactId>
  <version>1.2.0-SNAPSHOT</version>
  <packaging>pom</packaging>
  <name>Pantry</name>

  <modules>
    <module>stock</module>
    <module>web</module>
  </modules>

  <properties>
    <maven.compiler.release>21</maven.compiler.release>
    <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
    <spring-boot.version>3.3.5</spring-boot.version>
  </properties>

  <dependencyManagement>
    <dependencies>
      <!-- Spring Boot's versions, for every module. -->
      <dependency>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-dependencies</artifactId>
        <version>${spring-boot.version}</version>
        <type>pom</type>
        <scope>import</scope>
      </dependency>
      <dependency>
        <groupId>com.example</groupId>
        <artifactId>stock</artifactId>
        <version>${project.version}</version>
      </dependency>
    </dependencies>
  </dependencyManagement>

  <build>
    <pluginManagement>
      <plugins>
        <plugin>
          <groupId>org.springframework.boot</groupId>
          <artifactId>spring-boot-maven-plugin</artifactId>
          <version>${spring-boot.version}</version>
        </plugin>
        <plugin>
          <artifactId>maven-compiler-plugin</artifactId>
          <version>3.13.0</version>
          <configuration>
            <parameters>true</parameters>
          </configuration>
        </plugin>
        <plugin>
          <artifactId>maven-surefire-plugin</artifactId>
          <version>3.5.2</version>
        </plugin>
      </plugins>
    </pluginManagement>
  </build>

  <profiles>
    <profile>
      <id>dev</id>
      <activation>
        <activeByDefault>true</activeByDefault>
      </activation>
      <properties>
        <spring.profiles.active>dev</spring.profiles.active>
      </properties>
    </profile>
    <profile>
      <id>prod</id>
      <properties>
        <spring.profiles.active>prod</spring.profiles.active>
      </properties>
    </profile>
  </profiles>
</project>
EOF
cat >"$M/stock/pom.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <parent>
    <groupId>com.example</groupId>
    <artifactId>pantry</artifactId>
    <version>1.2.0-SNAPSHOT</version>
  </parent>

  <artifactId>stock</artifactId>
  <name>Pantry stock</name>

  <dependencies>
    <dependency>
      <groupId>org.springframework</groupId>
      <artifactId>spring-context</artifactId>
    </dependency>
    <dependency>
      <groupId>org.junit.jupiter</groupId>
      <artifactId>junit-jupiter</artifactId>
      <scope>test</scope>
    </dependency>
  </dependencies>
</project>
EOF
cat >"$M/web/pom.xml" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <parent>
    <groupId>com.example</groupId>
    <artifactId>pantry</artifactId>
    <version>1.2.0-SNAPSHOT</version>
  </parent>

  <artifactId>web</artifactId>
  <name>Pantry web</name>

  <dependencies>
    <dependency>
      <groupId>com.example</groupId>
      <artifactId>stock</artifactId>
    </dependency>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-web</artifactId>
    </dependency>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-configuration-processor</artifactId>
      <optional>true</optional>
    </dependency>
    <dependency>
      <groupId>org.springframework.boot</groupId>
      <artifactId>spring-boot-starter-test</artifactId>
      <scope>test</scope>
    </dependency>
  </dependencies>

  <build>
    <plugins>
      <plugin>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-maven-plugin</artifactId>
      </plugin>
    </plugins>
  </build>
</project>
EOF
cat >"$MS/Item.java" <<'EOF'
package com.example.pantry.stock;

/** Something on the shelves, how many of it, and the mark at which to buy more. */
public record Item(String name, int quantity, int low) {

    public boolean runningLow() {
        return quantity <= low;
    }
}
EOF
cat >"$MS/Shelves.java" <<'EOF'
package com.example.pantry.stock;

import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import org.springframework.stereotype.Component;

/** What the pantry holds. */
@Component
public class Shelves {

    private final Map<String, Item> items = new ConcurrentHashMap<>();

    public Shelves() {
        put(new Item("rice", 6, 2));
        put(new Item("olive oil", 1, 1));
        put(new Item("coffee", 0, 1));
    }

    public List<Item> all() {
        return items.values().stream().sorted(Comparator.comparing(Item::name)).toList();
    }

    public List<Item> toBuy() {
        return all().stream().filter(Item::runningLow).toList();
    }

    public Item take(String name, int amount) {
        Item item = items.get(name);
        if (item == null) {
            throw new IllegalArgumentException("No " + name + " in the pantry");
        }
        Item left = new Item(name, Math.max(0, item.quantity() - amount), item.low());
        put(left);
        return left;
    }

    public void put(Item item) {
        items.put(item.name(), item);
    }
}
EOF
cat >"$MT/ShelvesTest.java" <<'EOF'
package com.example.pantry.stock;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.Test;

class ShelvesTest {

    @Test
    void takingLeavesWhatIsLeft() {
        assertEquals(4, new Shelves().take("rice", 2).quantity());
    }

    @Test
    void nothingGoesBelowZero() {
        assertEquals(0, new Shelves().take("olive oil", 5).quantity());
    }

    @Test
    void listsWhatToBuy() {
        // An item at its low mark is to buy now; this test still counts only the empty ones.
        assertEquals(1, new Shelves().toBuy().size());
    }

    @Test
    void refusesWhatIsNotThere() {
        assertThrows(IllegalArgumentException.class, () -> new Shelves().take("saffron", 1));
    }
}
EOF
cat >"$MW/PantryApplication.java" <<'EOF'
package com.example.pantry.web;

import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.context.properties.EnableConfigurationProperties;

@SpringBootApplication(scanBasePackages = "com.example.pantry")
@EnableConfigurationProperties(PantryProperties.class)
public class PantryApplication {

    public static void main(String[] args) {
        SpringApplication.run(PantryApplication.class, args);
    }
}
EOF
cat >"$MW/PantryProperties.java" <<'EOF'
package com.example.pantry.web;

import org.springframework.boot.context.properties.ConfigurationProperties;

/** The pantry's settings, from application.properties. */
@ConfigurationProperties("pantry")
public record PantryProperties(String owner, int reorderAt) {
}
EOF
cat >"$MW/ShelfController.java" <<'EOF'
package com.example.pantry.web;

import com.example.pantry.stock.Item;
import com.example.pantry.stock.Shelves;
import java.util.List;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/shelves")
public class ShelfController {

    private final Shelves shelves;
    private final PantryProperties pantry;

    public ShelfController(Shelves shelves, PantryProperties pantry) {
        this.shelves = shelves;
        this.pantry = pantry;
    }

    @GetMapping
    public List<Item> all() {
        return shelves.all();
    }

    @GetMapping("/to-buy")
    public List<Item> toBuy() {
        return shelves.toBuy();
    }

    @PostMapping("/{name}/take")
    public Item take(@PathVariable String name) {
        return shelves.take(name, 1);
    }

    @GetMapping("/owner")
    public String owner() {
        return pantry.owner();
    }
}
EOF
cat >"$M/web/src/main/resources/application.properties" <<'EOF'
spring.application.name=pantry
server.port=8081
pantry.owner=Alex
pantry.reorder-at=2
EOF
cat >"$MWT/ShelfControllerTest.java" <<'EOF'
package com.example.pantry.web;

import static org.junit.jupiter.api.Assertions.assertEquals;

import com.example.pantry.stock.Shelves;
import org.junit.jupiter.api.Test;

class ShelfControllerTest {

    @Test
    void takesOneAtATime() {
        ShelfController controller = new ShelfController(new Shelves(), new PantryProperties("Alex", 2));
        assertEquals(5, controller.take("rice").quantity());
    }

    @Test
    void namesTheOwner() {
        ShelfController controller = new ShelfController(new Shelves(), new PantryProperties("Alex", 2));
        assertEquals("Alex", controller.owner());
    }
}
EOF
cat >"$M/README.md" <<'EOF'
# pantry

What is on the shelves, and what to buy: a Maven build of a library (stock) and a Spring Boot
application (web).

    mvn -pl web -am spring-boot:run
EOF
commit "$M" sam "Pantry: the shelves and their web app"
perl -pi -e 's/^server.port=8081$/server.port=8081\nspring.jackson.default-property-inclusion=non_null/' "$M/web/src/main/resources/application.properties"
commit "$M" alex "Leave empty fields out of the JSON"
echo "done"
