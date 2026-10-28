package demo;

import java.time.Duration;

import org.springframework.boot.context.properties.ConfigurationProperties;

/** Settings of the demo. */
@ConfigurationProperties(prefix = "demo")
public class DemoProperties {
    /** The name the demo greets. */
    private String name = "world";

    /** How many times to retry. */
    private int retries = 3;

    /** How long to wait. */
    private Duration timeout;

    /** How fast to go. */
    private Mode mode = Mode.CAREFUL;

    private final Greeting greeting = new Greeting();

    public enum Mode { CAREFUL, FAST }

    public static class Greeting {
        /** What comes before the name. */
        private String prefix = "Hello";

        public String getPrefix() {
            return prefix;
        }

        public void setPrefix(String prefix) {
            this.prefix = prefix;
        }
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    public int getRetries() {
        return retries;
    }

    public void setRetries(int retries) {
        this.retries = retries;
    }

    public Duration getTimeout() {
        return timeout;
    }

    public void setTimeout(Duration timeout) {
        this.timeout = timeout;
    }

    public Mode getMode() {
        return mode;
    }

    public void setMode(Mode mode) {
        this.mode = mode;
    }

    public Greeting getGreeting() {
        return greeting;
    }
}
