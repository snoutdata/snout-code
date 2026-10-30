//! The Spring model as the engine answers it for its `spring-app` fixture (recorded in
//! `tests/fixtures/spring-app.json`, with the folder as `{ROOT}`).

use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

use super::window::{BeanFilter, BeanRow, Column, EndpointFilter, Grouping, ProfileFilter, Source};
use super::*;
use crate::editor::{SpringItem, spring_items};

/// The recorded answers, with the folder `root` in their URIs (`file://{ROOT}/…`; a Windows
/// folder as `/C:/…`).
pub fn fixture(root: &Path) -> Value {
    let root = root.to_string_lossy().replace('\\', "/");
    let root = if root.starts_with('/') { root } else { format!("/{root}") };
    let text = include_str!("../../tests/fixtures/spring-app.json").replace("{ROOT}", &root);
    serde_json::from_str(&text).unwrap()
}

fn root() -> &'static Path {
    Path::new("/work/spring-app")
}

#[test]
fn reads_the_beans_and_the_endpoints() {
    let f = fixture(root());
    let modules = parse_beans(&f["beans"]);
    assert_eq!(modules.len(), 1);
    let m = &modules[0];
    assert_eq!(m.module, "spring-app");
    assert_eq!(m.beans.len(), 9);
    assert!(m.library_beans > 0, "the library's beans are counted");
    let clock = m.beans.iter().find(|b| b.name == "clock").unwrap();
    assert_eq!(clock.kind, "bean");
    assert_eq!(clock.method.as_deref(), Some("clock"));
    assert_eq!(clock.injected_count(), 1);
    assert_eq!(clock.stereotype_label(), "Bean");
    assert_eq!(clock.package(), "demo.config");
    let english = m.beans.iter().find(|b| b.name == "englishGreeting").unwrap();
    assert_eq!(english.stereotype_label(), "Service");
    let endpoints = parse_endpoints(&f["endpoints"]);
    assert_eq!(endpoints.len(), 3);
    let status = endpoints.iter().find(|e| e.path == "/status").unwrap();
    assert_eq!(status.methods_label(), "GET, HEAD");
    assert_eq!(status.handler_label(), "StatusController.status");
    assert_eq!(status.summary(), "GET, HEAD /status\nGET, HEAD /health");
    let echo = endpoints.iter().find(|e| e.path == "/api/echo").unwrap();
    assert!(echo.summary().contains("Consumes application/json"));
}

#[test]
fn marks_a_files_beans_injections_and_endpoints_by_line() {
    let f = fixture(root());
    let endpoints = parse_endpoints(&f["endpoints"]);
    let file = FileSpring::parse(&*snout_host::local(), &f["injections"]["GreetingController"], &endpoints);
    let marks: Vec<(u32, MarkKind)> = file.lines.iter().map(|l| (l.line, l.mark().unwrap())).collect();
    assert_eq!(
        marks,
        [
            (18, MarkKind::Bean),
            (24, MarkKind::Injection { problem: true }),
            (27, MarkKind::Injection { problem: false }),
            (30, MarkKind::Injection { problem: false }),
            (32, MarkKind::Injection { problem: true }),
            (39, MarkKind::Endpoint),
            (44, MarkKind::Endpoint),
        ]
    );
    let hello = file.line(39).unwrap();
    assert_eq!(hello.tooltip(), "GET /api/hello");
    assert!(file.line(24).unwrap().tooltip().contains("No bean of type 'AuditLog' found"));
}

#[test]
fn a_marks_menu_offers_the_places_to_go() {
    let f = fixture(root());
    let endpoints = parse_endpoints(&f["endpoints"]);
    let controller = FileSpring::parse(&*snout_host::local(), &f["injections"]["GreetingController"], &endpoints);
    // The ambiguous constructor parameter: a chooser of both greetings (and the clock and
    // the properties, which resolve to one each).
    let items = spring_items(controller.line(32).unwrap());
    let goes: Vec<&str> = items
        .iter()
        .filter_map(|i| match i {
            SpringItem::Go { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert!(goes.contains(&"frenchGreeting (FrenchGreeting)"), "{goes:?}");
    assert!(goes.contains(&"englishGreeting (EnglishGreeting)"), "{goes:?}");
    assert!(items.contains(&SpringItem::Heading("'greetings' could take any of".into())), "{items:?}");
    // The endpoint: its method and path, Copy Path, Show in Endpoints.
    let items = spring_items(controller.line(44).unwrap());
    assert_eq!(
        items,
        [
            SpringItem::Heading("POST /api/echo".into()),
            SpringItem::CopyPath("/api/echo".into()),
            SpringItem::ShowEndpoint("/api/echo".into()),
        ]
    );
    // A @Bean method: where it is injected; its parameter, the library's ObjectMapper.
    let config = FileSpring::parse(&*snout_host::local(), &f["injections"]["AppConfig"], &endpoints);
    let items = spring_items(config.line(11).unwrap());
    assert_eq!(items[0], SpringItem::Heading("Bean 'clock' is injected into".into()));
    let SpringItem::Go { label, detail, target } = &items[1] else { panic!("{items:?}") };
    assert_eq!(label, "GreetingController(clock)");
    assert_eq!(detail, "GreetingController.java:33");
    assert_eq!(target.file().unwrap().0, root().join("src/main/java/demo/web/GreetingController.java"));
    let items = spring_items(config.line(16).unwrap());
    let library = items
        .iter()
        .find_map(|i| match i {
            SpringItem::Go { target, detail, .. } if target.uri.is_none() => Some((target, detail)),
            _ => None,
        })
        .expect("the library's bean");
    assert_eq!(library.1, "library");
    assert!(library.0.class.as_deref().unwrap().ends_with("JacksonObjectMapperConfiguration"));
}

#[test]
fn the_beans_tab_groups_and_filters() {
    let f = fixture(root());
    let mut modules = parse_beans(&f["beans"]);
    // A profile on one bean and a Scala bean, for the filters.
    modules[0].beans[0].profiles = vec!["dev".into()];
    modules[0].beans[1].language = "scala".into();
    let none = HashSet::new();
    let all = window::bean_rows(&modules, &BeanFilter::default(), Grouping::Package, &none, &none);
    let groups: Vec<&str> = all
        .iter()
        .filter_map(|r| match r {
            BeanRow::Group { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(groups, ["demo", "demo.config", "demo.greeting", "demo.web"]);
    // Groups start closed (more than one): no bean rows until one opens.
    assert!(!all.iter().any(|r| matches!(r, BeanRow::Bean(_))));
    let open: HashSet<String> = ["m:spring-app:demo.greeting".to_owned()].into();
    let rows = window::bean_rows(&modules, &BeanFilter::default(), Grouping::Package, &open, &none);
    let names: Vec<String> = beans_in(&rows);
    assert_eq!(names, ["englishGreeting", "frenchGreeting"]);
    // A search opens every group and keeps what matches.
    let search = BeanFilter { query: "greeting".into(), ..Default::default() };
    let names = beans_in(&window::bean_rows(&modules, &search, Grouping::Package, &none, &none));
    assert_eq!(names, ["englishGreeting", "frenchGreeting", "greetingController"]);
    let services = BeanFilter { stereotype: Some("Service".into()), ..Default::default() };
    assert_eq!(beans_in(&window::bean_rows(&modules, &services, Grouping::Stereotype, &none, &none)).len(), 2);
    let dev = BeanFilter { profile: ProfileFilter::Named("dev".into()), ..Default::default() };
    assert_eq!(beans_in(&window::bean_rows(&modules, &dev, Grouping::Package, &none, &none)), ["appConfig"]);
    let scala = BeanFilter { source: Source::Scala, ..Default::default() };
    assert_eq!(beans_in(&window::bean_rows(&modules, &scala, Grouping::Package, &none, &none)), ["clock"]);
    let unprofiled = BeanFilter { profile: ProfileFilter::Unprofiled, ..Default::default() };
    assert_eq!(beans_in(&window::bean_rows(&modules, &unprofiled, Grouping::Package, &none, &none)).len(), 8);
    // A closed module hides its groups.
    let closed: HashSet<String> = ["m:spring-app".to_owned()].into();
    assert_eq!(window::bean_rows(&modules, &BeanFilter::default(), Grouping::Package, &none, &closed).len(), 1);
}

fn beans_in(rows: &[BeanRow]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| match r {
            BeanRow::Bean(b) => Some(b.name.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_endpoints_tab_filters_and_sorts() {
    let f = fixture(root());
    let all = parse_endpoints(&f["endpoints"]);
    let paths = |f: &EndpointFilter| window::endpoint_rows(&all, f).into_iter().map(|e| e.path).collect::<Vec<_>>();
    assert_eq!(paths(&EndpointFilter::default()), ["/api/echo", "/api/hello", "/status"]);
    let posts = EndpointFilter { method: Some("POST".into()), ..Default::default() };
    assert_eq!(paths(&posts), ["/api/echo"]);
    let by_handler = EndpointFilter { sort: Column::Handler, ascending: false, ..Default::default() };
    assert_eq!(paths(&by_handler), ["/status", "/api/hello", "/api/echo"]);
    let search = EndpointFilter { query: "health".into(), ..Default::default() };
    assert_eq!(paths(&search), ["/status"], "every path of a mapping is searched");
    let search = EndpointFilter { query: "greeting hello".into(), ..Default::default() };
    assert_eq!(paths(&search), ["/api/hello"]);
}

#[test]
fn reads_value_hints_and_knows_configuration_files() {
    let f = fixture(root());
    assert_eq!(parse_hints(&f["inlayHints"]), [(26, "= Howdy".to_owned()), (29, "= none (default)".to_owned())]);
    assert!(is_config_file(Path::new("/p/src/main/resources/application.properties")));
    assert!(is_config_file(Path::new("/p/application-dev.yml")));
    assert!(is_config_file(Path::new("/p/bootstrap.yaml")));
    assert!(!is_config_file(Path::new("/p/messages.properties")));
    assert!(!is_config_file(Path::new("/p/application.json")));
    assert_eq!(crate::lang::hub::language_id(Path::new("/p/application.yml")), "spring-boot-properties-yaml");
    assert_eq!(crate::lang::hub::language_id(Path::new("/p/application.properties")), "spring-boot-properties");
    assert_eq!(crate::lang::hub::language_id(Path::new("/p/App.java")), "java");
    assert_eq!(window::thousands(1242), "1,242");
    assert_eq!(window::thousands(1_565_000), "1,565,000");
    assert_eq!(window::thousands(15), "15");
}

#[test]
fn profile_modes_toggle_and_say_what_they_are() {
    use crate::spring::{ProfileInfo, ProfileMode, mode_label};
    let info = ProfileInfo::parse(&serde_json::json!({ "modules": [
        { "module": "app", "configActive": ["dev"], "known": [ { "name": "dev" }, { "name": "prod" } ] }
    ] }));
    assert_eq!(mode_label(&ProfileMode::Any, &info), "Any profile");
    assert_eq!(mode_label(&ProfileMode::Config, &info), "dev (from the files)");
    // Turning one on from the files' set starts from it.
    let next = ProfileMode::Config.toggled("prod", &info);
    assert_eq!(next, ProfileMode::Chosen(vec!["dev".into(), "prod".into()]));
    assert_eq!(next.toggled("dev", &info), ProfileMode::Chosen(vec!["prod".into()]));
    assert_eq!(mode_label(&ProfileMode::Chosen(vec![]), &info), "default");
    assert_eq!(next.params()["profiles"], serde_json::json!(["dev", "prod"]));
    assert_eq!(ProfileMode::Any.params()["mode"], "any");
    assert_eq!(info.known().iter().map(|k| k.name.as_str()).collect::<Vec<_>>(), ["dev", "prod"]);
    assert_eq!(crate::run::config_dialog::toggled_profiles("dev, local", "dev"), "local");
    assert_eq!(crate::run::config_dialog::toggled_profiles("local", "dev"), "local, dev");
}
