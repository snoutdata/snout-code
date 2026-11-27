//! Cypress runs from the Run machinery: the Tests window's and the Cypress window's buttons,
//! a spec's gutter icons, a `package.json` script that is `cypress run`. Each finds the
//! package's Node off the UI thread, adds the environment picked, and starts as a process
//! run whose reporter feeds the session's [`crate::playwright::report::Run`]. The fixes the
//! Tests window offers for a run that went wrong start here too, and run the specs again
//! once they end well.

use std::path::PathBuf;

use gpui::Context;

use super::node_run::project_node;
use super::{Launch, Outcome, Place, Runs, RunsEvent};
use crate::cypress::{Browser, Fix, Options};
use crate::jstest::{Runner, Scope};

impl Runs {
    /// The browser Cypress runs in for this project.
    pub fn cypress_browser(&self) -> Browser {
        self.project.cypress_browser.unwrap_or_default()
    }

    /// Run Cypress in `browser` from now on.
    pub fn set_cypress_browser(&mut self, browser: Browser, cx: &mut Context<Self>) {
        self.project.cypress_browser = Some(browser);
        self.save_project();
        cx.notify();
    }

    /// The environment's variables for a Cypress run, as Cypress reads them.
    fn cypress_vars(&self, cx: &Context<Self>) -> Vec<(String, String)> {
        let Some(project) = self.model.read(cx).project_root() else { return Vec::new() };
        let host = crate::host::host(cx);
        let vars =
            self.environment().map(|e| e.variables(|f| host.read_to_string(&project.join(f)).ok())).unwrap_or_default();
        crate::cypress::environment_vars(&vars)
    }

    /// Run `scope` of the package at `dir` with Cypress, into the Tests window: in the
    /// browser picked, showing it when `headed`, against the environment picked.
    pub fn start_cypress(&mut self, dir: PathBuf, scope: Scope, headed: bool, cx: &mut Context<Self>) {
        let Some(project) = self.model.read(cx).project_root() else {
            self.say(ts!("run-open-a-project-first"), cx);
            return;
        };
        let host = crate::host::host(cx);
        let browser = self.cypress_browser();
        let vars = self.cypress_vars(cx);
        let environment = self.environment().map(|e| e.name.clone());
        let at = dir.clone();
        let find = cx.background_executor().spawn(async move {
            let n = project_node(&*host, &at, &project)?;
            crate::cypress::run_launch(&*host, &at, &project, &n, &scope, &Options { browser, headed }, vars)
        });
        cx.spawn(async move |this, cx| {
            let found = find.await;
            let _ = this.update(cx, |this, cx| match found {
                Ok(launch) => {
                    if let Some(id) = this.start(Launch::Process(launch), cx)
                        && let Some(run) = this.session_mut(id).and_then(|s| s.playwright.as_mut())
                    {
                        run.runner = Some(Runner::Cypress);
                        run.dir = Some(dir);
                        run.environment = environment;
                        run.browser = browser;
                    }
                }
                Err(why) => this.say(why, cx),
            });
        })
        .detach();
    }

    /// Open the Cypress app on the package at `dir`, in the browser picked: to watch a spec
    /// run step by step, and to debug it in the browser's developer tools. One already open
    /// there stays the one: its Run tab comes forward.
    pub fn open_cypress(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let open = self.sessions.iter().rev().find(|s| {
            s.running()
                && s.title == ts!("cypress-open-cypress")
                && matches!(&s.launch, Launch::Process(p) if p.cwd == dir)
        });
        if let Some(id) = open.map(|s| s.id) {
            self.select(Place::Run, id, cx);
            cx.emit(RunsEvent::Show(Place::Run));
            self.say(ts!("cypress-the-cypress-app-is-already-open"), cx);
            return;
        }
        let Some(project) = self.model.read(cx).project_root() else { return };
        let host = crate::host::host(cx);
        let browser = self.cypress_browser();
        let vars = self.cypress_vars(cx);
        let find = cx.background_executor().spawn(async move {
            let n = project_node(&*host, &dir, &project)?;
            crate::cypress::open_launch(&*host, &dir, &n, browser, vars)
        });
        cx.spawn(async move |this, cx| {
            let found = find.await;
            let _ = this.update(cx, |this, cx| match found {
                Ok(launch) => {
                    this.start(Launch::Process(launch), cx);
                }
                Err(why) => this.say(why, cx),
            });
        })
        .detach();
    }

    /// Carry out `fix` for the Cypress run `run` (a session), then run it again once the
    /// fix is done.
    pub fn fix_cypress(&mut self, run: u64, fix: Fix, cx: &mut Context<Self>) {
        let Some(s) = self.session(run) else { return };
        let Some(dir) = s.playwright.as_ref().and_then(|r| r.dir.clone()) else { return };
        let args = match &s.launch {
            Launch::Process(p) => p.args.clone(),
            _ => Vec::new(),
        };
        match fix {
            Fix::UseBrowser(b) => {
                self.set_cypress_browser(b, cx);
                let (scope, headed) = crate::cypress::scope_of_args(&dir, &args);
                self.start_cypress(dir, scope, headed, cx);
            }
            Fix::RunAgain => self.rerun(run, cx),
            // The Tests window opens the dialog.
            Fix::EditEnvironments => {}
            Fix::UpdateCypress | Fix::InstallBinary | Fix::InstallRosetta => {
                let Some(project) = self.model.read(cx).project_root() else { return };
                let host = crate::host::host(cx);
                let find = cx.background_executor().spawn(async move {
                    let n = project_node(&*host, &dir, &project)?;
                    crate::cypress::fix_launch(&*host, &dir, &project, &n, fix)
                });
                cx.spawn(async move |this, cx| {
                    let found = find.await;
                    let _ = this.update(cx, |this, cx| match found {
                        Ok(launch) => {
                            if let Some(id) = this.start(Launch::Process(launch), cx) {
                                this.cypress_then.insert(id, run);
                            }
                        }
                        Err(why) => this.say(why, cx),
                    });
                })
                .detach();
            }
        }
    }

    /// Session `id` ended: when it was a fix that went well, run the specs again.
    pub(super) fn cypress_fix_ended(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(run) = self.cypress_then.remove(&id) else { return };
        let ok = self.session(id).and_then(|s| s.ended.as_ref()).is_some_and(|e| e.outcome == Outcome::Success);
        if ok {
            self.rerun(run, cx);
        }
    }
}
