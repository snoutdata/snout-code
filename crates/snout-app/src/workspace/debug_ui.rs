//! The workspace's side of the debugger's named actions: breakpoints at the caret (toggle,
//! enable, condition), View Breakpoints, Evaluate Expression, Run to Cursor, Pause, Rerun,
//! Rerun Failed Tests, and Run Gradle Task's picker.

use gpui::{AppContext as _, Context, Window};

use super::Workspace;
use crate::run::Launch;
use crate::run::debug_dialogs::{BreakpointsDialog, DebugDialogEvent, EvaluateDialog};
use crate::search::{Mode, SearchEvent, SearchView};

impl Workspace {
    fn say(&mut self, text: &str, cx: &mut Context<Self>) {
        self.model.update(cx, |m, cx| {
            m.message = text.to_owned();
            cx.notify();
        });
    }

    /// The editor in front and the line its caret is on, in a file that can have breakpoints.
    fn breakpoint_target(&self, window: &Window, cx: &mut Context<Self>) -> Option<(std::path::PathBuf, u32)> {
        let editor = self.active_editor(window, cx)?;
        let e = editor.read(cx);
        Some((e.path.clone(), e.breakpoint_line(cx)?))
    }

    /// Toggle Breakpoint (⌘F8): set or clear the breakpoint on the caret's line.
    pub fn toggle_breakpoint_at_caret(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.breakpoint_target(window, cx) {
            Some((path, line)) => self.runs.update(cx, |r, cx| r.toggle_breakpoint(&path, line, cx)),
            None => self.say("Breakpoints go in a Java, Kotlin or Scala file", cx),
        }
    }

    /// Toggle Breakpoint Enabled (⌥⌘F8): keep the caret line's breakpoint without stopping
    /// there, or let it stop again.
    pub fn toggle_breakpoint_enabled_at_caret(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((path, line)) = self.breakpoint_target(window, cx) else {
            return self.say("Breakpoints go in a Java, Kotlin or Scala file", cx);
        };
        if !self.runs.update(cx, |r, cx| r.toggle_breakpoint_enabled(&path, line, cx)) {
            self.say("There is no breakpoint on this line: ⌘F8 sets one", cx);
        }
    }

    /// Edit Breakpoint Condition (⌥⇧⌘F8): the condition popup at the caret.
    pub fn edit_breakpoint_condition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.active_editor(window, cx) else { return };
        if !editor.update(cx, |e, cx| e.edit_condition_at_caret(window, cx)) {
            self.say("Breakpoints go in a Java, Kotlin or Scala file", cx);
        }
    }

    /// Go to Spring Bean or Usages…: the Spring mark of the caret's line, as a click on it.
    pub fn spring_at_caret(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.active_editor(window, cx) else { return };
        if !editor.update(cx, |e, cx| e.spring_menu_at_caret(window, cx)) {
            self.say("No Spring bean, injection or endpoint on this line", cx);
        }
    }

    /// View Breakpoints (⇧⌘F8).
    pub fn open_breakpoints(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.breakpoints_dialog.is_some() {
            return;
        }
        let root = self.model.read(cx).project_root().unwrap_or_default();
        let runs = self.runs.clone();
        let dialog = cx.new(|cx| BreakpointsDialog::new(runs, root, window, cx));
        cx.subscribe_in(&dialog, window, |this, _, e: &DebugDialogEvent, window, cx| {
            this.breakpoints_dialog = None;
            match e {
                DebugDialogEvent::Close => window.focus(&this.focus, cx),
                DebugDialogEvent::Open(path, line) => this.open_file(path.clone(), Some((*line, 0)), false, window, cx),
            }
            cx.notify();
        })
        .detach();
        let focus = gpui::Focusable::focus_handle(dialog.read(cx), cx);
        window.focus(&focus, cx);
        self.breakpoints_dialog = Some(dialog);
        cx.notify();
    }

    /// Evaluate Expression (⌥F8): the selection, or the word at the caret, to start with.
    pub fn open_evaluate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.evaluate_dialog.is_some() {
            return;
        }
        let initial = self
            .active_editor(window, cx)
            .map(|e| {
                let s = e.read(cx).state.read(cx);
                let range = s.selected_range();
                if range.is_empty() {
                    let text = s.text().to_string();
                    crate::editing::word_at(&text, range.start).unwrap_or_default()
                } else {
                    s.text().slice(range).to_string()
                }
            })
            .unwrap_or_default();
        let runs = self.runs.clone();
        let dialog = cx.new(|cx| EvaluateDialog::new(runs, initial, window, cx));
        cx.subscribe_in(&dialog, window, |this, _, _: &DebugDialogEvent, window, cx| {
            this.evaluate_dialog = None;
            window.focus(&this.focus, cx);
            cx.notify();
        })
        .detach();
        let d = dialog.clone();
        cx.defer_in(window, move |_, window, cx| d.update(cx, |d, cx| d.focus_input(window, cx)));
        self.evaluate_dialog = Some(dialog);
        cx.notify();
    }

    /// Run to Cursor (⌥F9): the stopped program goes on to the caret's line.
    pub fn run_to_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.runs.read(cx).paused_debug() else {
            return self.say("Run to Cursor works while the debugger is stopped", cx);
        };
        let Some((path, line)) = self.breakpoint_target(window, cx) else {
            return self.say("Put the caret on a line of a Java, Kotlin or Scala file", cx);
        };
        self.runs.update(cx, |r, cx| r.run_to_cursor(id, &path, line, cx));
    }

    /// Pause Program: stop the running program where it is.
    pub fn pause_program(&mut self, cx: &mut Context<Self>) {
        let runs = self.runs.read(cx);
        match runs.live_debug().filter(|id| !runs.session(*id).and_then(|s| s.debug.as_ref()).is_some_and(|d| d.paused))
        {
            Some(id) => self.runs.update(cx, |r, cx| r.step(id, "pause", cx)),
            None => self.say("Nothing to pause: no program is running in the debugger", cx),
        }
    }

    /// Rerun (⌘R): the session the Run, Debug or Build window shows last.
    pub fn rerun_last(&mut self, cx: &mut Context<Self>) {
        match self.runs.read(cx).last_session() {
            Some(id) => self.runs.update(cx, |r, cx| r.rerun(id, cx)),
            None => self.say("Nothing ran yet to run again", cx),
        }
    }

    /// Rerun Failed Tests (⌥⌘R): the tests that failed in the last test run.
    pub fn rerun_failed_tests(&mut self, cx: &mut Context<Self>) {
        let runs = self.runs.read(cx);
        let found = runs.last_tests().filter(|id| {
            runs.session(*id).is_some_and(|s| !s.running() && s.tests.as_ref().is_some_and(|t| t.counts().1 > 0))
        });
        match found {
            Some(id) => self.runs.update(cx, |r, cx| r.rerun_failed(id, cx)),
            None => self.say("No failed tests to run again", cx),
        }
    }

    /// Run Gradle Task… (⌃⌥G): every task of the project's builds, picked by name.
    pub fn open_task_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let runs = self.runs.read(cx);
        let mut commands: Vec<crate::search::Command> = Vec::new();
        for b in &runs.gradle.builds {
            for project in &b.projects {
                for (_, tasks) in &project.groups {
                    for t in tasks {
                        let name = if runs.gradle.builds.len() > 1 {
                            format!("{}: {}", b.build, t.path)
                        } else {
                            t.path.clone()
                        };
                        commands.push(crate::search::Command {
                            name: name.into(),
                            action: Box::new(crate::menus::RunTask {
                                build: Some(b.build.clone()),
                                task: t.path.clone(),
                            }),
                            keys: None,
                        });
                    }
                }
            }
        }
        commands.sort_by(|a, b| a.name.cmp(&b.name));
        commands.dedup_by(|a, b| a.name == b.name);
        if commands.is_empty() {
            let loading = runs.gradle.loading;
            self.runs.update(cx, |r, cx| r.load_gradle_tasks(false, cx));
            return self.say(
                if loading {
                    "The Gradle tasks are being read; try again in a moment"
                } else {
                    "No Gradle tasks yet: the project's build is being read"
                },
                cx,
            );
        }
        let model = self.model.clone();
        let view = cx.new(|cx| SearchView::new(model, Default::default(), Mode::Tasks, window, cx));
        view.update(cx, |v, cx| v.set_commands(commands, cx));
        cx.subscribe_in(&view, window, move |this, _, e: &SearchEvent, window, cx| {
            this.search = None;
            window.focus(&this.focus, cx);
            if let SearchEvent::Run(action) = e {
                window.dispatch_action(action.boxed_clone(), cx);
            }
            cx.notify();
        })
        .detach();
        let v = view.clone();
        cx.defer_in(window, move |_, window, cx| v.update(cx, |v, cx| v.focus_input(window, cx)));
        self.search = Some(view);
        cx.notify();
    }

    /// Spring Active Profiles… (⌃⌥P): any profile, the files' ones, or each known profile
    /// on or off, picked by name.
    pub fn open_profile_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use crate::menus::SetSpringProfiles;
        use crate::spring::ProfileMode;
        let spring = self.spring.read(cx);
        let (mode, info) = (spring.profile_mode.clone(), spring.profile_info.clone());
        let on = |name: &str| match &mode {
            ProfileMode::Chosen(p) => p.iter().any(|x| x == name),
            ProfileMode::Config => info.active().unwrap_or_default().iter().any(|x| x == name),
            ProfileMode::Any => false,
        };
        let mark = |b: bool| b.then(|| "On".to_owned());
        let files = info.config_active();
        let mut commands = vec![
            crate::search::Command {
                name: "Any profile: every bean may be there".into(),
                action: Box::new(SetSpringProfiles(ProfileMode::Any)),
                keys: mark(mode == ProfileMode::Any),
            },
            crate::search::Command {
                name: format!(
                    "As the application files say ({})",
                    if files.is_empty() { "default".to_owned() } else { files.join(", ") }
                )
                .into(),
                action: Box::new(SetSpringProfiles(ProfileMode::Config)),
                keys: mark(mode == ProfileMode::Config),
            },
        ];
        for k in info.known() {
            commands.push(crate::search::Command {
                name: k.name.clone().into(),
                action: Box::new(SetSpringProfiles(mode.toggled(&k.name, &info))),
                keys: mark(on(&k.name)),
            });
        }
        let model = self.model.clone();
        let view = cx.new(|cx| SearchView::new(model, Default::default(), Mode::Profiles, window, cx));
        view.update(cx, |v, cx| v.set_commands(commands, cx));
        cx.subscribe_in(&view, window, move |this, _, e: &SearchEvent, window, cx| {
            this.search = None;
            window.focus(&this.focus, cx);
            if let SearchEvent::Run(action) = e {
                window.dispatch_action(action.boxed_clone(), cx);
            }
            cx.notify();
        })
        .detach();
        let v = view.clone();
        cx.defer_in(window, move |_, window, cx| v.update(cx, |v, cx| v.focus_input(window, cx)));
        self.search = Some(view);
        cx.notify();
    }

    /// Run one Gradle task, in the Build window.
    pub fn run_task(&mut self, build: Option<String>, task: String, cx: &mut Context<Self>) {
        self.runs.update(cx, |r, cx| {
            r.start(Launch::Task { build, task }, cx);
        });
    }
}
