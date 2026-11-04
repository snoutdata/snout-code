//! Playwright runs from the Run machinery: a configuration of the Playwright kind, a gutter
//! icon, the Tests window's buttons. Each finds the project's Node and its test runner off
//! the UI thread, adds the environment picked, then starts as a process run whose reporter
//! feeds the session's [`crate::playwright::report::Run`].

use std::path::{Path, PathBuf};

use gpui::Context;
use snout_host::Host;

use super::config::{Environment, Kind, RunConfig};
use super::process::ProcessLaunch;
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
        let env_name = self.environment().map(|e| e.name.clone());
        let file = match &scope {
            Scope::At { file, .. } => Some(file.clone()),
            _ => None,
        };
        self.start_node_tool(
            file,
            move |host, root, n, mut vars| {
                vars.extend(env);
                let mut launch = test_launch(host, root, n, &scope, options, &extra, vars)?;
                if let Some(t) = title {
                    launch.title = t;
                }
                Ok(launch)
            },
            move |this, started, cx| match started {
                Ok(id) => {
                    if let Some(run) = this.session_mut(id).and_then(|s| s.playwright.as_mut()) {
                        run.environment = env_name;
                    }
                }
                Err(why) => this.say(why, cx),
            },
            cx,
        );
    }

    /// Start a program of the open Playwright project (the one `file` is in, when given) on
    /// the Node it asks for, with the environment's variables: `build` makes its launch off
    /// the UI thread from the host, the project's folder, the Node and the variables; `then`
    /// hears the session it started as, or why it did not start.
    pub fn start_node_tool(
        &mut self,
        file: Option<PathBuf>,
        build: impl FnOnce(&dyn Host, &Path, &node::Node, Vec<(String, String)>) -> Result<ProcessLaunch, String>
        + Send
        + 'static,
        then: impl FnOnce(&mut Self, Result<u64, String>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.model.read(cx).project_root() else {
            then(self, Err("Open a Playwright project first".into()), cx);
            return;
        };
        let host = crate::host::host(cx);
        let environment = self.environment().cloned();
        let find = cx.background_executor().spawn(async move {
            // The Playwright project the file is in, else the open one.
            let root = file
                .as_deref()
                .and_then(|f| crate::playwright::root_of(&*host, f, Some(&project)))
                .unwrap_or_else(|| project.clone());
            let wanted = node::wanted(&*host, &root, Some(&project));
            let all = node::discover(&*host);
            let n = node::choose(&wanted, &all)
                .cloned()
                .ok_or("No Node fits this project: download it from the bar above the editor first.".to_owned())?;
            let vars =
                environment.map(|e| e.variables(|f| host.read_to_string(&root.join(f)).ok())).unwrap_or_default();
            build(&*host, &root, &n, vars)
        });
        cx.spawn(async move |this, cx| {
            let found = find.await;
            let _ = this.update(cx, |this, cx| {
                let started = found.and_then(|launch| {
                    this.start(Launch::Process(launch), cx).ok_or_else(|| "It did not start".to_owned())
                });
                then(this, started, cx);
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
