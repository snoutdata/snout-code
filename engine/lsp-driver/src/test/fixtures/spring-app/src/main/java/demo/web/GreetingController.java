package demo.web;

import java.time.Clock;

import demo.DemoProperties;
import demo.audit.AuditLog;
import demo.greeting.GreetingService;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api")
public class GreetingController {
    private final GreetingService greetings;
    private final Clock clock;
    private final DemoProperties properties;

    @Autowired
    private AuditLog audit;

    @Value("${demo.greeting.prefix:Hi}")
    private String prefix;

    @Value("${demo.banner:none}")
    private String banner;

    public GreetingController(GreetingService greetings, Clock clock, DemoProperties properties) {
        this.greetings = greetings;
        this.clock = clock;
        this.properties = properties;
    }

    @GetMapping("/hello")
    public String hello(@RequestParam String name) {
        return prefix + " " + greetings.greet(name) + " at " + clock.instant();
    }

    @PostMapping(value = "/echo", consumes = "application/json", produces = "application/json")
    public String echo(@RequestBody String body) {
        audit.record(body);
        return body;
    }
}
