package demo.web;

import org.springframework.stereotype.Controller;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestMethod;
import org.springframework.web.bind.annotation.ResponseBody;

@Controller
public class StatusController {
    @RequestMapping(value = {"/status", "/health"}, method = {RequestMethod.GET, RequestMethod.HEAD})
    @ResponseBody
    public String status() {
        return "up";
    }
}
