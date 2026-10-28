package demo.config;

import java.time.Clock;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

@Configuration
public class AppConfig {
    @Bean
    public Clock clock() {
        return Clock.systemUTC();
    }

    @Bean
    public String jsonSample(ObjectMapper mapper) throws Exception {
        return mapper.writeValueAsString(java.util.Map.of("ok", true));
    }
}
