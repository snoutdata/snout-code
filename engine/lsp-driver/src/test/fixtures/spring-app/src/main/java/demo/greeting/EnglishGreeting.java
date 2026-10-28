package demo.greeting;

import org.springframework.stereotype.Service;

@Service
public class EnglishGreeting implements GreetingService {
    @Override
    public String greet(String name) {
        return "Hello, " + name;
    }
}
