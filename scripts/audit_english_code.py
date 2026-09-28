"""Verify source translation removed Chinese without changing other code lines."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HAN = re.compile(r"[\u3400-\u9fff]")
ORIGINAL_CHINESE = re.compile(r"[\u3400-\u9fff\uff1a\uff1b\uff0c\u3002\uff01\uff1f\u201c\u201d]")
PROTECTED = {
    "apps/aicore-dashboard/src/client.r",
    "tests/Cargo.toml.model.should.delete",
    "aicored-once-output.txt",
}


def files() -> list[str]:
    raw = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
    )
    return [item.decode() for item in raw.split(b"\0") if item]


def main() -> None:
    checked = 0
    modified = 0
    issues: list[str] = []
    for relative in files():
        path = ROOT / relative
        if not path.is_file():
            continue
        if (
            relative.startswith("docs/")
            or path.suffix.lower() in {".md", ".txt"}
            or relative.endswith(".realPC")
            or relative in PROTECTED
        ):
            continue
        try:
            current = path.read_bytes().decode("utf-8-sig")
        except UnicodeDecodeError:
            continue
        checked += 1
        if HAN.search(current):
            issues.append(f"{relative}: Chinese text remains")
        previous = subprocess.run(
            ["git", "show", f"HEAD:{relative}"],
            cwd=ROOT,
            capture_output=True,
            check=False,
        )
        if previous.returncode != 0:
            continue
        before = previous.stdout.decode("utf-8-sig").splitlines()
        after = current.splitlines()
        if before == after:
            continue
        modified += 1
        if len(before) != len(after):
            issues.append(f"{relative}: line count changed")
            continue
        for line_number, (old, new) in enumerate(zip(before, after), 1):
            if old != new and not ORIGINAL_CHINESE.search(old):
                issues.append(f"{relative}:{line_number}: non-Chinese line changed")
    print(f"Checked {checked} non-document source files; {modified} differ from HEAD")
    for issue in issues:
        print(issue)
    print(f"Issues: {len(issues)}")
    if issues:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
