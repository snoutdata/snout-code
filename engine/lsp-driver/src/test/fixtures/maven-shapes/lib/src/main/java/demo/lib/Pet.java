package demo.lib;

import lombok.Getter;
import lombok.RequiredArgsConstructor;
import org.apache.commons.lang3.StringUtils;

@Getter
@RequiredArgsConstructor
public class Pet {
    private final String name;

    public String shout() {
        var loud = StringUtils.upperCase(name);
        return loud + "!" + (Version.VALUE.isEmpty() ? "?" : "");
    }
}
