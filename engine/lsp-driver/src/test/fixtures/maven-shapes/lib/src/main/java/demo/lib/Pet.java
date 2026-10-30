package demo.lib;

import lombok.Getter;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;

@Getter
@RequiredArgsConstructor
@Slf4j
public class Pet {
    private final String name;

    public String shout() {
        var loud = StringUtils.upperCase(name);
        LOGGER.info("{} shouts", name);
        return loud + "!" + (Version.VALUE.isEmpty() ? "?" : "");
    }
}
