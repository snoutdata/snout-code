use super::*;

fn files(query: &str, files: &[&str]) -> Vec<String> {
    let p = Pattern::new(query);
    let mut hits: Vec<(Match, &str)> = files.iter().filter_map(|f| p.match_path(f).map(|m| (m, *f))).collect();
    best_first(&mut hits);
    hits.into_iter().map(|(_, f)| f.to_owned()).collect()
}

fn names(query: &str, names: &[&str]) -> Vec<String> {
    let p = Pattern::new(query);
    let mut hits: Vec<(Match, &str)> = names.iter().filter_map(|n| p.match_name(n).map(|m| (m, *n))).collect();
    best_first(&mut hits);
    hits.into_iter().map(|(_, n)| n.to_owned()).collect()
}

fn tier(query: &str, name: &str) -> Option<Tier> {
    Pattern::new(query).match_name(name).map(|m| m.tier)
}

#[test]
fn the_tiers_go_from_as_typed_to_loose() {
    assert_eq!(tier("user", "User"), Some(Tier::Exact));
    assert_eq!(tier("user", "UserService"), Some(Tier::Prefix));
    assert_eq!(tier("us", "UserService"), Some(Tier::Prefix));
    assert_eq!(tier("serv", "UserService"), Some(Tier::WordStarts));
    assert_eq!(tier("US", "UserService"), Some(Tier::WordStarts));
    assert_eq!(tier("usse", "UserService"), Some(Tier::WordStarts));
    assert_eq!(tier("ervi", "UserService"), Some(Tier::Inside));
    assert_eq!(tier("usrsvc", "UserService"), Some(Tier::Loose));
    assert_eq!(tier("xyz", "UserService"), None);
}

#[test]
fn word_starts_know_camel_case_capitals_digits_and_separators() {
    assert_eq!(word_starts(&"getName".chars().collect::<Vec<_>>()), [0, 3]);
    assert_eq!(word_starts(&"HTTPServer".chars().collect::<Vec<_>>()), [0, 4]);
    assert_eq!(word_starts(&"Base64Encoder".chars().collect::<Vec<_>>()), [0, 4, 6]);
    assert_eq!(word_starts(&"run_gradle-task now".chars().collect::<Vec<_>>()), [0, 4, 11, 16]);
    assert_eq!(tier("hs", "HTTPServer"), Some(Tier::WordStarts));
    assert_eq!(tier("gn", "getName"), Some(Tier::WordStarts));
    assert_eq!(tier("tc", "TokenController"), Some(Tier::WordStarts));
    assert_eq!(tier("tokcon", "TokenController"), Some(Tier::WordStarts));
}

#[test]
fn names_rank_by_tier_then_by_length() {
    let all = ["UserServiceImpl", "AbstractUserService", "UserService", "User", "UsersView"];
    assert_eq!(names("user", &all), ["User", "UsersView", "UserService", "UserServiceImpl", "AbstractUserService"]);
    assert_eq!(names("US", &all)[..2], ["UserService", "UserServiceImpl"]);
}

#[test]
fn every_word_must_match_in_any_order() {
    let all = ["Run Gradle Task…", "Run Configuration…", "Gradle Sync"];
    assert_eq!(names("task run", &all), ["Run Gradle Task…"]);
    assert_eq!(names("gradle", &all), ["Gradle Sync", "Run Gradle Task…"]);
}

#[test]
fn wildcards_stand_for_whatever_is_there() {
    assert_eq!(tier("token*ler", "TokenController"), Some(Tier::Exact));
    assert_eq!(tier("*Controller", "TokenController"), Some(Tier::Exact));
    assert_eq!(tier("tok*con", "TokenController"), Some(Tier::Prefix));
    assert_eq!(tier("ken*con", "TokenController"), Some(Tier::Inside));
    assert_eq!(tier("*con", "TokenController"), Some(Tier::WordStarts));
    assert_eq!(tier("ok*rol", "TokenController"), Some(Tier::Inside));
    assert_eq!(tier("t?ken", "TokenController"), Some(Tier::Prefix));
    assert_eq!(files("token*.ja", &["src/TokenController.java", "src/Tokens.kt"]), ["src/TokenController.java"]);
    assert!(Pattern::new("* ?").is_empty());
}

#[test]
fn a_file_is_matched_by_its_name_before_its_folders() {
    let all = ["user/Settings.java", "app/UserSettings.java", "app/User.java", "app/user.ts"];
    assert_eq!(files("user", &all), ["app/user.ts", "app/User.java", "app/UserSettings.java", "user/Settings.java"]);
    let p = Pattern::new("user");
    assert_eq!(p.match_path("app/User.java").unwrap().tier, Tier::Exact, "the extension aside");
    assert_eq!(p.match_path("user/Settings.java").unwrap().tier, Tier::InFolders);
    assert_eq!(p.match_path("app/User.java").unwrap().indices, [4, 5, 6, 7], "counted in the whole path");
}

#[test]
fn a_slash_names_folders() {
    let all = ["web/src/App.tsx", "app/src/App.java", "web/src/main.tsx"];
    assert_eq!(files("web/app", &all), ["web/src/App.tsx"]);
    assert_eq!(files("src/app", &all), ["web/src/App.tsx", "app/src/App.java"]);
    assert_eq!(files("web/", &all), ["web/src/App.tsx", "web/src/main.tsx"]);
    assert_eq!(files("w/s/main", &all), ["web/src/main.tsx"]);
}

#[test]
fn letters_across_folders_still_find_the_file() {
    assert_eq!(files("srcmain", &["src/main.rs", "lib.rs"]), ["src/main.rs"]);
    assert_eq!(files("mainrs", &["src/main.rs", "status.rs"]), ["src/main.rs"]);
}

#[test]
fn a_line_at_the_end_is_where_to_open() {
    let p = Pattern::new("Foo.java:12");
    assert_eq!(p.line(), Some((11, 0)));
    assert!(p.match_path("src/Foo.java").is_some());
    assert_eq!(Pattern::new("foo:12:5").line(), Some((11, 4)));
    assert_eq!(Pattern::new("foo").line(), None);
    assert_eq!(Pattern::new("a:b").line(), None);
}

#[test]
fn the_caller_boosts_within_a_tier() {
    let p = Pattern::new("app");
    let mut hits = vec![
        (p.match_path("build/App.java").unwrap(), "build"),
        (p.match_path("src/App.java").unwrap().boosted(1), "src"),
        (p.match_path("src/Apple.java").unwrap().boosted(9), "apple"),
    ];
    best_first(&mut hits);
    let order: Vec<&str> = hits.iter().map(|h| h.1).collect();
    assert_eq!(order, ["src", "build", "apple"], "the tier first, then the boost");
}

#[test]
fn capitals_as_typed_count() {
    assert_eq!(names("Foo", &["foo", "Foo"]), ["Foo", "foo"], "first among equals");
    assert_eq!(names("foo", &["Foo", "foo"]), ["Foo", "foo"], "small letters take either");
    assert_eq!(tier("US", "UsersView"), Some(Tier::Loose), "a capital only on a capital");
    assert_eq!(tier("Serv", "observer"), Some(Tier::Loose));
    assert_eq!(tier("serv", "observer"), Some(Tier::Inside));
}

#[test]
fn filters_leave_out_the_loose_matches() {
    let p = Pattern::new("fl");
    assert!(p.filter("feature/login"), "the starts of words");
    assert!(!p.filter("fault"), "only loosely");
    assert!(Pattern::new("").filter("anything"));
    assert!(Pattern::new("JDK").filter("download a jdk"), "a capital finds a small letter in a list");
    assert!(Pattern::new("Src").filter_path("build/src/a.rs"));
}

#[test]
fn places_tell_the_projects_code_from_tests_and_builds() {
    assert_eq!(Place::of("src/main/java/App.java"), Place::Source);
    assert_eq!(Place::of("src/main/java/com/acme/build/Builder.java"), Place::Source, "a package named build");
    assert_eq!(Place::of("src/test/java/AppTest.java"), Place::Test);
    assert_eq!(Place::of("web/src/app.spec.ts"), Place::Test);
    assert_eq!(Place::of("AppIT.java"), Place::Test);
    assert_eq!(Place::of("IT.java"), Place::Source);
    assert_eq!(Place::of("build/generated/App.java"), Place::Generated);
    assert_eq!(Place::of("web/node_modules/react/index.js"), Place::Generated);
    assert_eq!(Place::of("../shared/src/A.java"), Place::Outside);
    assert!(Place::Source > Place::Test && Place::Test > Place::Outside && Place::Outside > Place::Generated);
}

#[test]
fn find_in_files_takes_a_star_by_a_word_as_a_wildcard() {
    assert_eq!(wildcard_regex("get*Name").as_deref(), Some("get[^\\n]*?Name"));
    assert_eq!(wildcard_regex("Token*").as_deref(), Some("Token[^\\n]*?"));
    assert_eq!(wildcard_regex("a.b*(c)").as_deref(), Some("a\\.b[^\\n]*?\\(c\\)"));
    assert_eq!(wildcard_regex("import java.util.*"), None);
    assert_eq!(wildcard_regex("/**"), None);
    assert_eq!(wildcard_regex("a * b"), None);
    assert_eq!(wildcard_regex("plain"), None);
    assert_eq!(literal_runs("get*Name"), ["get", "Name"]);
    assert_eq!(literal_runs("java.util.*"), ["java.util.*"]);
}
