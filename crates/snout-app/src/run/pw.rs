//! Playwright runs from the Run machinery: a configuration of the Playwright kind, a gutter
//! icon, the Tests window's buttons. Each finds the project's Node and its test runner off
//! the UI thread, adds the environment picked, then starts as a process run whose reporter
//! feeds the session's [`crate::playwright::report::Run`].

use gpui::Context;

use super::config::{Environment, Kind, RunConfig};
use super::{Launch, Runs};
use crate::lang::node;
use crate::playwright::run::{Options, Scope, test_launch};

impl Runs {
    /// The environment runs go against, when one is picked.
    pub fn environment(&self) -> Option<&Environment> {
        let name = self.project.environment.as_deref()?;
        self.project.environments.iter().find(|e| e.name == name)
    }

    /// Pick the environment runs go against (none: the configuration's own `baseURL`).
    pub fn select_environment(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        self.project.environment = name;
        self.save_project();
        cx.notify();
    }

    /// Keep the project's environments as edited, and which one is picked.
    pub fn set_environments(&mut self, list: Vec<Environment>, selected: Option<String>, cx: &mut Context<Self>) {
        self.project.environment = selected.filter(|n| list.iter().any(|e| &e.name == n));
        self.project.environments = list;
        self.save_project();
        cx.notify();
    }

    /// The configurations a Playwright project starts with: all its tests, and those of
    /// each of its projects (browsers).
    pub fn set_playwright_configs(&mut self, projects: &[String], cx: &mut Context<Self>) {
        let mut list = vec![RunConfig {
            id: "playwright:all".into(),
            kind: Kind::Playwright,
            name: "All tests".into(),
            ..Default::default()
        }];
        for p in projects {
            list.push(RunConfig {
                id: format!("playwright:project:{p}"),
                kind: Kind::Playwright,
                name: format!("Tests in {p}"),
                args: vec![format!("--project={p}")],
                ..Default::default()
            });
        }
        if self.engine_configs != list {
            self.engine_configs = list;
            cx.notify();
        }
    }

    /// Run a configuration of the Playwright kind.
    pub(super) fn start_playwright_config(&mut self, config: RunConfig, options: Options, cx: &mut Context<Self>) {
        let env: Vec<(String, String)> = config.env.clone().into_iter().collect();
        let mut extra = config.args.clone();
        if let Some(task) = config.task.as_deref().filter(|t| !t.trim().is_empty()) {
            extra.extend(super::config::split_args(task));
        }
        self.start_playwright_with(Scope::All, options, extra, env, Some(config.name.clone()), cx);
    }

    /// Run `scope` of the open Playwright project.
    pub fn start_playwright(&mut self, scope: Scope, options: Options, cx: &mut Context<Self>) {
        self.start_playwright_with(scope, options, Vec::new(), Vec::new(), None, cx);
    }

    fn start_playwright_with(
        &mut self,
        scope: Scope,
        options: Options,
        extra: Vec<String>,
        env: Vec<(String, String)>,
        title: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.model.read(cx).project_root() else {
            self.say("Open a Playwright project to run its tests", cx);
            return;
        };
        let host = crate::host::host(cx);
        let environment = self.environment().cloned();
        let env_name = environment.as_ref().map(|e| e.name.clone());
        let find = cx.background_executor().spawn(async move {
            // The Playwright project the scope is in: a file's nearest, else the open one.
            let root = match &scope {
                Scope::At { file, .. } => crate::playwright::root_of(&*host, file, Some(&project)),
                _ => None,
            }
            .unwrap_or_else(|| project.clone());
            let wanted = node::wanted(&*host, &root, Some(&project));
            let all = node::discover(&*host);
            let n = node::choose(&wanted, &all)
                .cloned()
                .ok_or("No Node fits this project: download it from the bar above the editor first.".to_owned())?;
            let mut vars =
                environment.map(|e| e.variables(|f| host.read_to_string(&root.join(f)).ok())).unwrap_or_default();
            vars.extend(env);
            let mut launch = test_launch(&*host, &root, &n, &scope, options, &extra, vars)?;
            if let Some(t) = title {
                launch.title = t;
            }
            Ok::<_, String>(launch)
        });
        cx.spawn(async move |this, cx| {
            let found = find.await;
            let _ = this.update(cx, |this, cx| match found {
                Ok(launch) => {
                    if let Some(id) = this.start(Launch::Process(launch), cx)
                        && let Some(run) = this.session_mut(id).and_then(|s| s.playwright.as_mut())
                    {
                        run.environment = env_name;
                    }
                }
                Err(why) => this.say(why, cx),
            });
        })
        .detach();
    }
}

impl Runs {
    /// A finished Playwright run, as `run` says it went: for the screenshots, which cannot
    /// wait on a process.
    #[doc(hidden)]
    pub fn adopt_playwright(
        &mut self,
        title: &str,
        mut run: crate::playwright::report::Run,
        cx: &mut Context<Self>,
    ) -> u64 {
        let cwd = self.project_root.clone().unwrap_or_default();
        let launch = Launch::Process(crate::run::process::ProcessLaunch {
            title: title.to_owned(),
            program: "node".into(),
            args: Vec::new(),
            cwd,
            env: Vec::new(),
            place: super::Place::Run,
            report: true,
        });
        let id = self.adopt(launch, "playwright", cx);
        run.finish();
        let elapsed = std::time::Duration::from_millis(run.millis.unwrap_or(0));
        if let Some(s) = self.session_mut(id) {
            let failed = run.counts().failed > 0;
            s.playwright = Some(run);
            s.ended = Some(super::Ended {
                outcome: if failed { super::Outcome::Failed } else { super::Outcome::Success },
                elapsed,
                exit_code: Some(i64::from(failed)),
                error: None,
            });
        }
        cx.notify();
        id
    }
}
