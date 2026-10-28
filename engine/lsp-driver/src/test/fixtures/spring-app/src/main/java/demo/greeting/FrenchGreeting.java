package demo.greeting;

import org.springframework.stereotype.Service;

@Service
public class FrenchGreeting implements GreetingService {
    @Override
    public String greet(String name) {
        return "Bonjour, " + name;
    }
}
