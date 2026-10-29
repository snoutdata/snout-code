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
    "projects-project-view": ("projects-project-view-light", None),
    "projects-new-spring": ("projects-new-spring-light", (580, 260, 2300, 1540)),
    "projects-new-dependencies": ("projects-new-deps-light", (580, 260, 2300, 1540)),
    "editor-search-everywhere": ("editor-search-everywhere-light", None),
    "editor-command-palette": ("editor-command-palette-light", None),
    "editor-context-menu": ("editor-context-menu-light", None),
    "java-completion": ("java-completion-light", None),
    "java-hover": ("java-hover-light", None),
    "java-problems": ("java-problems-light", None),
    "java-quick-fix": ("java-quick-fix-light", None),
    "java-rename": ("java-rename-light", None),
    "java-references": ("java-references-light", None),
    "parameter-hints": ("java-param-hints-light", None),
    "structure": ("java-structure-light", None),
    "call-hierarchy": ("java-call-hierarchy-light", None),
    "go-to-class": ("java-go-to-class-light", None),
    "extract-method": ("java-extract-method-light", None),
    "gradle-window": ("gradle-window-light", None),
    "run-and-debug-run-window": ("run-window-light", None),
    "run-and-debug-title-bar": ("run-window-light", (1960, 0, 2880, 76)),
    "run-and-debug-tests": ("run-tests-light", None),
    "run-and-debug-debugger": ("debug-paused-light", None),
    "run-and-debug-debugger-dark": ("debug-paused-dark", None),
    "spring-injections": ("spring-injections-light", None),
    "spring-beans": ("spring-beans-light", None),
    "spring-endpoints": ("spring-endpoints-light", None),
    "spring-yml-completion": ("spring-yml-completion-light", None),
    "processes-window": ("processes-light", None),
    "git-view": ("git-view-light", None),
    "git-commit": ("git-commit-light", None),
    "git-discard": ("git-discard-light", None),
    "git-error": ("git-error-light", None),
    "git-clean": ("git-clean-light", None),
    "claude-code-console": ("claude-console-light", None),
    "claude-code-live-strike": ("claude-live-strike-light", None),
    "claude-code-live-typing": ("claude-live-typing-light", None),
    "claude-code-changes": ("claude-changes-light", None),
    "claude-code-inline": ("claude-inline-light", None),
    "remote-ssh-connect": ("remote-connect-light", None),
    "remote-ssh-picker": ("remote-picker-light", None),
    "remote-ssh-project": ("remote-project-light", None),
    "remote-ssh-reconnecting": ("remote-reconnecting-light", None),
    "settings-and-keymap-appearance": ("settings-appearance-light", (580, 320, 2300, 1480)),
    "settings-and-keymap-appearance-dark": ("settings-appearance-dark", (580, 320, 2300, 1480)),
    "settings-and-keymap-keymap": ("settings-keymap-light", (580, 320, 2300, 1480)),
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
