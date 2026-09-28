"""Translate tracked source comments without changing executable prefixes."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

from local_translation import translate


ROOT = Path(__file__).resolve().parents[1]
HAN = re.compile(r"[\u3400-\u9fff]")
COMMENT_START = re.compile(r"//[/!]?|#|<!--")
PROTECTED = {
    "apps/aicore-dashboard/src/client.r",
    "tests/Cargo.toml.model.should.delete",
    "aicored-once-output.txt",
}


def source_paths() -> list[Path]:
    raw = subprocess.check_output(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
    )
    paths = []
    for item in raw.split(b"\0"):
        if not item:
            continue
        relative = item.decode()
        path = ROOT / relative
        if (
            relative.startswith("docs/")
            or path.suffix.lower() in {".md", ".txt"}
            or relative.endswith(".realPC")
            or relative in PROTECTED
        ):
            continue
        if path.is_file():
            paths.append(path)
    return paths


def source_bytes(path: Path) -> bytes:
    relative = path.relative_to(ROOT).as_posix()
    previous = subprocess.run(
        ["git", "show", f"HEAD:{relative}"],
        cwd=ROOT,
        capture_output=True,
        check=False,
    )
    return previous.stdout if previous.returncode == 0 else path.read_bytes()


def translate_line(line: str) -> str:
    replacements = {
        "ci.yml \u5173\u952e\u7247\u6bb5": "key sections of ci.yml",
        "tests/Cargo.toml \u4e2d\u7684 [[test]]": "[[test]] entries in tests/Cargo.toml",
        "\u5404\u7c7b\u6d4b\u8bd5\u6570\u91cf": "test counts by type",
        "ComputerSnapshot \u5b9a\u4e49": "ComputerSnapshot definition",
        "System::new / refresh \u8c03\u7528": "System::new / refresh calls",
        "timestamp \u7528\u6cd5": "timestamp usage",
        "\u9519\u8bef\uff1a\u627e\u4e0d\u5230 manifest": "Error: manifest not found",
        "\u9519\u8bef\uff1a\u65e0\u6cd5\u4ece": "Error: cannot read from",
        " \u8bfb\u53d6 workspace.package.version": " workspace.package.version",
        "top_processes \u672a\u5305\u542b code/sleep\uff1bnames": "top_processes does not include code/sleep; names",
    }
    for original, english in replacements.items():
        line = line.replace(original, english)
    if not HAN.search(line):
        return line
    if 'echo "' in line or '"warning:' in line or '"\u9519\u8bef' in line:
        raise RuntimeError(f"Untranslated runtime string: {line}")
    marker = COMMENT_START.search(line)
    if marker and HAN.search(line[marker.end() :]):
        spaces = re.match(r"\s*", line[marker.end() :]).group()
        end = marker.end() + len(spaces)
        prefix, body = line[:end], line[end:]
    else:
        prefix = re.match(r"\s*(?:\*|;)?\s*", line).group()
        body = line[len(prefix) :]
    english = translate(body, "zh_en")
    if HAN.search(english):
        raise RuntimeError(f"Chinese remained after translation: {line}")
    return prefix + english


def main() -> None:
    changed = 0
    translated = 0
    for path in source_paths():
        try:
            original = source_bytes(path)
            text = original.decode("utf-8-sig")
        except UnicodeDecodeError:
            continue
        if not HAN.search(text):
            continue
        raw = path.read_bytes()
        bom = raw.startswith(b"\xef\xbb\xbf")
        lines = raw.decode("utf-8-sig").splitlines(keepends=True)
        previous = text.splitlines(keepends=True)
        if len(lines) != len(previous):
            raise RuntimeError(f"Line count changed: {path}")
        for index, line in enumerate(previous):
            if not HAN.search(line):
                continue
            newline = "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
            content = line[: -len(newline)] if newline else line
            lines[index] = translate_line(content) + newline
            translated += 1
        path.write_bytes((b"\xef\xbb\xbf" if bom else b"") + "".join(lines).encode())
        changed += 1
        if changed % 25 == 0:
            print(f"Translated {changed} files / {translated} lines", flush=True)
    print(f"Translated {changed} source files / {translated} lines", flush=True)


if __name__ == "__main__":
    main()
