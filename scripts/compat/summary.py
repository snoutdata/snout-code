"""summary.py <name> <cell dir>: one line of scripts/compat-matrix.sh's table, from the
lsp-driver's report and the model the engine wrote."""
import json, sys, glob, os
name, d = sys.argv[1], sys.argv[2]
try:
    r = json.load(open(os.path.join(d, "report.json")))
except Exception as e:
    print(f"{name:28} NO REPORT ({open(os.path.join(d,'driver.err')).read()[-300:]!r})"); sys.exit()
models = glob.glob(os.path.join(d, "cache/jdtls/*/model.json"))
jdk, status, reason = "?", "?", ""
if models:
    m = json.load(open(models[0]))
    b = m["gradleBuilds"][0]
    status, reason = b.get("status"), b.get("reason", "")
    jdk = b.get("javaVersion") or "?"
    if m.get("unsupportedBuilds"):
        reason = "outside the range" + (": " + reason if reason else "")
checks = " ".join(f'{c["name"]}={"ok" if c.get("pass") else "FAIL"}' for c in r.get("checks", []))
d_ = r.get("diagnostics", {})
print(f'{name:28} sync={status:7} jdk={jdk:10} ready={r.get("serviceReadyMillis")}ms errors={d_.get("errors")} {checks} {reason}')
for c in r.get("checks", []):
    if not c.get("pass"):
        print("   ", json.dumps(c)[:600])
for k in d_.get("errorKinds", [])[:3]:
    print("    error:", k.get("sample"))
