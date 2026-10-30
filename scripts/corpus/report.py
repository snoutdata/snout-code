"""The corpus's results (scripts/corpus.sh).

  report.py names <projects.txt> [name…]            the projects' names (all, or those named,
                                                    checked), as a JSON array
  report.py select <projects.txt> [name…]           the projects (all, or those named), one per
                                                    line, their fields separated by tabs
  report.py result <dir> <hint> <wall s> <name> <repository> <commit> <jdk> <tool> <shape>
                                                    <dir>/result.json, from the lsp-driver's report,
                                                    the model the engine wrote and the logs
  report.py table <out.md> <out.json> <result.json>…   the table (Markdown) and all results (JSON)

<hint> is how the run ended: "ran", "timeout:<minutes>", "fetch-failed", or "missing" (for a
project whose CI job left no result).
"""
import glob
import json
import os
import re
import sys

UNRESOLVED_IMPORT = "268435846"  # IProblem.ImportNotFound


def projects(path):
    out = []
    for line in open(path):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 5)
        if len(parts) < 5:
            raise SystemExit(f"{path}: not a project line: {line}")
        name, repo, commit, jdk, tool = parts[:5]
        out.append({"name": name, "repository": repo, "commit": commit, "jdk": jdk, "tool": tool,
                    "shape": parts[5] if len(parts) > 5 else ""})
    return out


def load(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def tail(path, pattern=None, n=1):
    try:
        lines = open(path, errors="replace").read().splitlines()
    except OSError:
        return []
    if pattern:
        lines = [l for l in lines if re.search(pattern, l)]
    return lines[-n:]


def died(d):
    """Why a run ended early: the engine's last word, else the client's exception."""
    why = tail(os.path.join(d, "engine.log"), r"^(snout-engine: |unknown option|Exception|Error)", 1) \
        or tail(os.path.join(d, "driver.log"), r"^Exception|^\S+Exception: ", 1)
    return why[0][:300] if why else ""


def first_line(text):
    return (text or "").strip().splitlines()[0][:300] if (text or "").strip() else ""


def seconds(ms):
    return None if ms is None or ms < 0 else round(ms / 1000, 1)


def result(d, hint, wall, meta):
    r = dict(meta)
    r["wallSeconds"] = round(float(wall), 1)
    report = load(os.path.join(d, "report.json"))
    model = load(os.path.join(d, "model.json"))
    engine_log = os.path.join(d, "engine.log")
    reasons = []

    if model:
        builds = model.get("gradleBuilds") or []
        r["builds"] = len(builds)
        r["buildsSynced"] = sum(1 for b in builds if b.get("status") == "synced")
        r["modules"] = len(model.get("modules") or [])
        r["syncSeconds"] = seconds(model.get("syncMillis"))
        r["versions"] = sorted({("maven " + b["mavenVersion"]) if b.get("mavenVersion") else ("gradle " + str(b.get("gradleVersion")))
                                for b in builds})
        r["failedBuilds"] = [{"build": f.get("build"), "reason": f.get("reason") or first_line(f.get("error")),
                              "needsJdk": f.get("needsJdk")} for f in model.get("failedBuilds") or []]
        r["unsupported"] = [u.get("note") for u in model.get("unsupportedBuilds") or []]
        if model.get("syncError"):
            reasons.append("sync failed: " + first_line(model["syncError"]))
        for f in r["failedBuilds"]:
            reasons.append(f"{f['build']}: {f['reason']}")
        for b in builds:
            if b.get("status") == "syncing":
                reasons.append(f"{b.get('id')}: still syncing")
        if not builds and not model.get("syncError"):
            reasons.append("no Gradle or Maven build found")
        elif r["modules"] == 0 and not reasons:
            reasons.append("the sync gave no modules")
    if report:
        d_ = report.get("diagnostics") or {}
        kinds = d_.get("errorKinds") or []
        r["errors"] = d_.get("errors")
        r["filesWithErrors"] = d_.get("filesWithErrors")
        r["unresolvedTypes"] = sum(k["count"] for k in kinds
                                   if "cannot be resolved to a type" in k["kind"] or k["kind"].startswith(UNRESOLVED_IMPORT + " "))
        r["topErrors"] = [{"count": k["count"], "sample": k.get("sample")} for k in kinds[:5]]
        r["readySeconds"] = seconds(report.get("serviceReadyMillis"))
        r["projectReadySeconds"] = seconds(report.get("projectReadyMillis"))
        r["buildSeconds"] = seconds(report.get("buildMillis"))
        r["peakJdtlsRssMb"] = report.get("peakJdtlsRssMb")

    if hint == "missing":
        r["status"], r["reason"] = "error", "no result: its job ended before the script did (see the job's log)"
    elif hint == "fetch-failed":
        r["status"], r["reason"] = "error", "could not fetch the commit"
    elif hint.startswith("timeout"):
        last = tail(engine_log, r"snout-engine: ", 1)
        r["status"] = "timeout"
        r["reason"] = f"did not finish in {hint.split(':', 1)[1]} minutes" + ("; last: " + last[0][:200] if last else "")
    elif model is None:
        r["status"] = "error"
        why = died(d)
        r["reason"] = "no model" + (": " + why if why else "")
    elif reasons:
        r["status"], r["reason"] = "fail", "; ".join(reasons)[:600]
    elif report is None:
        r["status"] = "error"
        why = died(d)
        r["reason"] = "the language server did not finish" + (": " + why if why else "")
    else:
        r["status"], r["reason"] = "pass", ""
    with open(os.path.join(d, "result.json"), "w") as f:
        json.dump(r, f, indent=1)
    print(f"{r['name']:32} {r['status']:7} sync={r.get('syncSeconds')}s modules={r.get('modules')} "
          f"errors={r.get('errors')} unresolved={r.get('unresolvedTypes')} {r['reason']}")


def cell(v):
    return "" if v is None else str(v).replace("|", "\\|").replace("\n", " ")


def table(md, out, paths):
    results = [r for r in (load(p) for p in paths) if r]
    passed = sum(1 for r in results if r["status"] == "pass")
    total = len(results)
    rate = round(100 * passed / total) if total else 0
    with open(out, "w") as f:
        json.dump({"passed": passed, "total": total, "passRate": rate, "projects": results}, f, indent=1)
    lines = [
        "# Corpus",
        "",
        f"**{passed} of {total} projects synced ({rate}%).** A project passes when every build in it syncs, "
        "the sync gives modules, and the Java language server imports them. Errors are the Java errors "
        "the language server reports after building the whole project; unresolved types are those "
        "among them that name a type or import it cannot find.",
        "",
        "| Project | Build | JDK | Result | Sync (s) | Ready (s) | Modules | Errors | Unresolved types | Reason or first error |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for r in results:
        first = r.get("reason") or next((e["sample"] for e in r.get("topErrors") or []), "")
        lines.append("| " + " | ".join(cell(v) for v in [
            f"[{r['name']}]({r['repository']}/tree/{r['commit']})", ", ".join(r.get("versions") or [r["tool"]]),
            r["jdk"], r["status"], r.get("syncSeconds"), r.get("readySeconds"), r.get("modules"),
            r.get("errors"), r.get("unresolvedTypes"), (first or "")[:300]]) + " |")
    with open(md, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"{passed} of {total} passed ({rate}%)")


if __name__ == "__main__":
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd in ("names", "select"):
        chosen = projects(args[0])
        unknown = set(args[1:]) - {p["name"] for p in chosen}
        if unknown:
            raise SystemExit("not in the corpus: " + ", ".join(sorted(unknown)))
        chosen = [p for p in chosen if not args[1:] or p["name"] in args[1:]]
        if cmd == "names":
            print(json.dumps([p["name"] for p in chosen]))
        for p in chosen if cmd == "select" else []:
            print("\t".join(p[k] for k in ["name", "repository", "commit", "jdk", "tool", "shape"]))
    elif cmd == "result":
        d, hint, wall = args[:3]
        keys = ["name", "repository", "commit", "jdk", "tool", "shape"]
        result(d, hint, wall, dict(zip(keys, args[3:9])))
    elif cmd == "table":
        table(args[0], args[1], args[2:])
    else:
        raise SystemExit(__doc__)
