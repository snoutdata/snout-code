//! The build: Gradle's tasks and Maven's goals, running one, and reading the build again.

use gpui::{Context, Window};
use serde_json::{Value, json};

use super::{Capability, Domain, Effect, Input, Run, missing, need, text_of};
use crate::claude_ide::server::Reply;
use crate::claude_ide::tools::{self, INVALID_PARAMS, TOOL_ERROR};
use crate::run::Launch;
use crate::workspace::Workspace;

pub const ALL: &[Capability] = &[
    Capability {
        name: "listBuildTasks",
        domain: Domain::Build,
        effect: Effect::Read,
        description: "The project's Gradle tasks and Maven goals, by build and module. Empty until the build has been \
                      read: try again shortly.",
        input: Input::None,
        run: Run::Own(list),
        consent: None,
    },
    Capability {
        name: "runBuildTask",
        domain: Domain::Build,
        effect: Effect::Change,
        description: "Run a Gradle task or Maven goals in the Build window, where the user sees them. For Gradle give \
                      task, as listBuildTasks has its path; for Maven give goals, and module. Answers with the run's \
                      id; follow it with listRuns and getRunOutput.",
        input: Input::Own(run_input),
        run: Run::Own(run),
        consent: Some(|a| match a["goals"].as_str() {
            Some(goals) => format!("run the Maven goals {goals}"),
            None => format!("run the Gradle task {}", text_of(a, "task")),
        }),
    },
    Capability {
        name: "syncBuild",
        domain: Domain::Build,
        effect: Effect::Change,
        description: "Read the project's build again, after changing a build file: Gradle's sync, or Maven's refresh.",
        input: Input::None,
        run: Run::Own(sync),
        consent: Some(|_| "sync the project's build".to_owned()),
    },
];

fn run_input() -> Value {
    tools::schema(
        json!({
            "task": { "type": "string", "description": "A Gradle task's path, such as :app:test" },
            "goals": { "type": "string", "description": "Maven goals or phases, such as clean verify" },
            "build": { "type": "string", "description": "The build, from listBuildTasks, when there are several" },
            "module": { "type": "string", "description": "A Maven module's project, such as :app. The whole build by default" },
        }),
        &[],
    )
}

fn list(ws: &mut Workspace, _: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let runs = ws.runs.clone();
    runs.update(cx, |r, cx| {
        if r.gradle.builds.is_empty() && !r.gradle.loading {
            r.load_gradle_tasks(false, cx);
        }
        if r.maven.builds.is_empty() && !r.maven.loading {
            r.load_maven_goals(false, cx);
        }
    });
    let r = runs.read(cx);
    let gradle: Vec<Value> = r
        .gradle
        .builds
        .iter()
        .map(|b| {
            let projects: Vec<Value> = b
                .projects
                .iter()
                .map(|p| {
                    let tasks: Vec<&str> =
                        p.groups.iter().flat_map(|(_, tasks)| tasks.iter()).map(|t| t.path.as_str()).collect();
                    json!({ "project": p.path, "tasks": tasks })
                })
                .collect();
            json!({ "build": b.build, "error": b.error, "projects": projects })
        })
        .collect();
    let maven: Vec<Value> = r
        .maven
        .builds
        .iter()
        .map(|b| {
            let modules: Vec<Value> = b
                .modules
                .iter()
                .map(|m| json!({ "project": m.project, "name": m.name, "lifecycle": m.lifecycle }))
                .collect();
            json!({ "build": b.build, "error": b.error, "modules": modules })
        })
        .collect();
    reply.ok(tools::json_text(&json!({
        "loading": r.gradle.loading || r.maven.loading,
        "gradle": gradle,
        "maven": maven,
    })));
}

fn run(ws: &mut Workspace, a: &Value, reply: Reply, _: &mut Window, cx: &mut Context<Workspace>) {
    let runs = ws.runs.clone();
    let launch = if let Some(goals) = need(a, "goals") {
        let r = runs.read(cx);
        let asked = a["build"].as_str();
        let build = r.maven.builds.iter().find(|b| asked.is_none_or(|n| n == b.build));
        let Some(build) = build else {
            return reply.error(INVALID_PARAMS, "No such Maven build. listBuildTasks has them.", None);
        };
        let project = a["module"].as_str().unwrap_or(":").to_owned();
        let Some(module) = build.modules.iter().find(|m| m.project == project) else {
            return reply.error(INVALID_PARAMS, &format!("No Maven module {project}. listBuildTasks has them."), None);
        };
        Launch::Maven { build: build.build.clone(), project, module: module.module.clone(), goals: goals.to_owned() }
    } else if let Some(task) = need(a, "task") {
        Launch::Task { build: a["build"].as_str().map(str::to_owned), task: task.to_owned() }
    } else {
        return missing(reply, "task or goals");
    };
    match runs.update(cx, |r, cx| r.start(launch, cx)) {
        Some(id) => reply.ok(tools::json_text(&json!({ "id": id }))),
        None => reply.error(TOOL_ERROR, "It did not start. The Build window may say why.", None),
    }
}

fn sync(ws: &mut Workspace, _: &Value, reply: Reply, window: &mut Window, cx: &mut Context<Workspace>) {
    let Some(project) = ws.model.read(cx).project_root() else {
        return reply.error(TOOL_ERROR, "No project is open", None);
    };
    if crate::project_model::project_has_gradle(&project, cx) {
        window.dispatch_action(Box::new(crate::menus::SyncGradle), cx);
    }
    window.dispatch_action(Box::new(crate::menus::RefreshMaven), cx);
    reply.ok(tools::text("Reading the build again. listBuildTasks has what it finds."));
}
