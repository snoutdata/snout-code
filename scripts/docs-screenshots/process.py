"""Crop, resize (max 1600 px wide) and optimise the raw shots into docs/images
(run by scripts/docs-screenshots.sh).

Usage: process.py <raw dir> <out dir> [name ...]
Crops are in the raw shots' pixels (2x of the 1440x900 window: 2880x1800).
"""
import sys
from pathlib import Path

from PIL import Image

# output name: (raw name, crop box (left, top, right, bottom) or None)
SPEC = {
    "getting-started-welcome": ("welcome-light", None),
    "getting-started-quit": ("quit-dialog-light", (760, 120, 2120, 860)),
    "projects-project-view": ("projects-project-view-light", None),
    "projects-new-spring": ("projects-new-spring-light", (580, 260, 2300, 1540)),
    "projects-new-dependencies": ("projects-new-deps-light", (580, 260, 2300, 1540)),
    "editor-search-everywhere": ("editor-search-everywhere-light", None),
    "editor-command-palette": ("editor-command-palette-light", None),
    "editor-context-menu": ("editor-context-menu-light", None),
    "editor-replace-in-files": ("editor-replace-in-files-light", None),
    "java-completion": ("java-completion-light", None),
    "java-hover": ("java-hover-light", None),
    "java-problems": ("java-problems-light", None),
    "java-quick-fix": ("java-quick-fix-light", None),
    "java-problem-hover": ("java-problem-hover-light", None),
    "java-rename": ("java-rename-light", None),
    "java-references": ("java-references-light", None),
    "parameter-hints": ("java-param-hints-light", None),
    "structure": ("java-structure-light", None),
    "call-hierarchy": ("java-call-hierarchy-light", None),
    "java-nav-link": ("java-nav-link-light", None),
    "java-usages": ("java-usages-light", None),
    "java-implementations": ("java-implementations-light", None),
    "go-to-class": ("java-go-to-class-light", None),
    "extract-method": ("java-extract-method-light", None),
    "gradle-window": ("gradle-window-light", None),
    "run-and-debug-run-window": ("run-window-light", None),
    "run-and-debug-title-bar": ("run-window-light", (1960, 0, 2880, 76)),
    "run-and-debug-tests": ("run-tests-light", None),
    "run-and-debug-debugger": ("debug-paused-light", None),
    "run-and-debug-debugger-dark": ("debug-paused-dark", None),
    "run-and-debug-breakpoints": ("breakpoints-light", (710, 390, 2170, 1410)),
    "run-and-debug-evaluate": ("evaluate-light", (840, 140, 2040, 760)),
    "spring-injections": ("spring-injections-light", None),
    "spring-beans": ("spring-beans-light", None),
    "spring-endpoints": ("spring-endpoints-light", None),
    "spring-profiles": ("spring-profiles-light", None),
    "spring-yml-completion": ("spring-yml-completion-light", None),
    "processes-window": ("processes-light", None),
    "git-view": ("git-view-light", None),
    "git-commit": ("git-commit-light", None),
    "git-discard": ("git-discard-light", None),
    "git-error": ("git-error-light", None),
    "git-clean": ("git-clean-light", None),
    "blame-column": ("blame-column-light", None),
    "blame-column-dark": ("blame-column-dark", None),
    "blame-card": ("blame-card-light", None),
    "blame-highlight": ("blame-highlight-light", None),
    "blame-previous": ("blame-previous-light", None),
    "blame-hints": ("blame-hints-light", None),
    "blame-agents": ("blame-agents-light", None),
    "agents-console": ("claude-console-light", None),
    "agents-console-start": ("agents-start-light", (720, 1230, 2800, 1745)),
    "agents-presence": ("agents-presence-light", None),
    "agents-presence-dark": ("agents-presence-dark", None),
    "agents-review": ("agents-review-light", None),
    "agents-changes": ("claude-changes-light", None),
    "agents-inline": ("claude-inline-light", None),
    "remote-ssh-connect": ("remote-connect-light", None),
    "remote-ssh-picker": ("remote-picker-light", None),
    "remote-ssh-project": ("remote-project-light", None),
    "remote-ssh-reconnecting": ("remote-reconnecting-light", None),
    "settings-and-keymap-appearance": ("settings-appearance-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-appearance-dark": ("settings-appearance-dark", (580, 240, 2300, 1560)),
    "settings-and-keymap-keymap": ("settings-keymap-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-editor": ("settings-editor-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-terminal": ("settings-terminal-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-terminal-shell": ("settings-terminal-shell-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-agents": ("settings-agents-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-project": ("settings-project-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-overridden": ("settings-overridden-light", (580, 240, 2300, 1560)),
    "settings-and-keymap-json-problems": ("settings-json-problems-light", None),
    "settings-and-keymap-json-completion": ("settings-json-completion-light", None),
    "project-structure-project": ("structure-project-light", (420, 220, 2460, 1580)),
    "project-structure-modules": ("structure-modules-light", (420, 220, 2460, 1580)),
    "project-structure-modules-dark": ("structure-modules-dark", (420, 220, 2460, 1580)),
    "project-structure-dependencies": ("structure-dependencies-light", (420, 220, 2460, 1580)),
    "project-structure-conflicts": ("structure-conflicts-light", (420, 220, 2460, 1580)),
    "project-structure-sdks": ("structure-sdks-light", (420, 220, 2460, 1580)),
    "project-structure-facets": ("structure-facets-light", (420, 220, 2460, 1580)),
}


def main():
    raw, out = Path(sys.argv[1]), Path(sys.argv[2])
    names = sys.argv[3:] or list(SPEC)
    out.mkdir(parents=True, exist_ok=True)
    for name in names:
        src, box = SPEC[name]
        path = raw / f"{src}.png"
        if not path.exists():
            print(f"missing {src}")
            continue
        img = Image.open(path).convert("RGB")
        if box:
            img = img.crop(box)
        if img.width > 1600:
            h = round(img.height * 1600 / img.width)
            img = img.resize((1600, h), Image.LANCZOS)
        q = img.quantize(colors=256, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.NONE)
        dest = out / f"{name}.png"
        q.save(dest, optimize=True)
        print(f"{name}: {img.width}x{img.height} {dest.stat().st_size // 1024} KB")


main()
