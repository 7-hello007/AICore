"""Generate Markdown language pairs using project-local translation models."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

from local_translation import translate


ROOT = Path(__file__).resolve().parents[1]
HAN = re.compile(r"[\u3400-\u9fff]")
INLINE = re.compile(r"`[^`\n]+`|\]\([^\n)]+\)")
MARKER = re.compile(r"^(\s*(?:#{1,6}\s+|[-*+]\s+|\d+[.)]\s+|>\s*)?)(.*)$")


def should_translate(text: str, direction: str) -> bool:
    if direction == "zh_en":
        return bool(HAN.search(text))
    return bool(re.search(r"[A-Za-z]{2,}", INLINE.sub("", text)))


def translate_document(source: str, direction: str) -> str:
    lines = source.splitlines()
    output: list[str] = []
    in_fence = False
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            output.append(line)
            index += 1
            continue
        if in_fence or not line.strip() or re.fullmatch(r"\s*[-=*]{3,}\s*", line):
            output.append(line)
            index += 1
            continue
        if line.lstrip().startswith("|") and line.rstrip().endswith("|"):
            cells = line.split("|")
            for cell_index in range(1, len(cells) - 1):
                cell = cells[cell_index]
                if should_translate(cell, direction) and not re.fullmatch(r"\s*:?-+:?\s*", cell):
                    cells[cell_index] = translate(cell, direction)
            output.append("|".join(cells))
            index += 1
            continue
        match = MARKER.match(line)
        prefix, body = match.groups()
        is_heading = bool(re.match(r"\s*#{1,6}\s", line))
        is_list = bool(re.match(r"\s*(?:[-*+]\s|\d+[.)]\s)", line))
        if is_heading:
            output.append(prefix + (translate(body, direction) if should_translate(body, direction) else body))
            index += 1
            continue
        if is_list:
            index += 1
            continuation = []
            while index < len(lines):
                next_line = lines[index]
                if (
                    not next_line.strip()
                    or next_line.lstrip().startswith(("```", "|", "#"))
                    or re.match(r"\s*(?:[-*+]\s|\d+[.)]\s)", next_line)
                ):
                    break
                continuation.append(next_line.strip())
                index += 1
            joined = " ".join([body.strip(), *continuation])
            output.append(prefix + (translate(joined, direction) if should_translate(joined, direction) else joined))
            continue
        paragraph = [line.strip()]
        index += 1
        while index < len(lines):
            next_line = lines[index]
            if (
                not next_line.strip()
                or next_line.lstrip().startswith(("```", "|", "#"))
                or re.fullmatch(r"\s*[-=*]{3,}\s*", next_line)
                or re.match(r"\s*(?:[-*+]\s|\d+[.)]\s)", next_line)
            ):
                break
            paragraph.append(next_line.strip())
            index += 1
        joined = " ".join(paragraph)
        output.append(prefix + (translate(joined, direction) if should_translate(joined, direction) else joined))
    return "\n".join(output) + "\n"


def original(path: Path) -> str:
    relative = path.relative_to(ROOT).as_posix()
    result = subprocess.run(
        ["git", "show", f"HEAD:{relative}"], cwd=ROOT, capture_output=True, check=False
    )
    return (
        result.stdout.decode("utf-8-sig")
        if result.returncode == 0
        else path.read_text(encoding="utf-8-sig")
    )


def localize_links(source: str, document: Path) -> str:
    def replace(match: re.Match[str]) -> str:
        target, anchor = match.group(1), match.group(2) or ""
        if "://" in target or target.endswith(".zh-CN.md"):
            return match.group()
        linked = (document.parent / target).resolve()
        companion = linked.with_name(linked.stem + ".zh-CN.md")
        return f"]({target[:-3]}.zh-CN.md{anchor})" if companion.is_file() else match.group()

    return re.sub(r"\]\(([^)#]+\.md)(#[^)]*)?\)", replace, source)


def main() -> None:
    files = [ROOT / "README.md", ROOT / "destination.md"]
    files.extend(sorted((ROOT / "docs").rglob("*.md")))
    files = [path for path in files if not path.name.endswith(".zh-CN.md")]
    if "--links-only" in sys.argv:
        for path in files:
            companion = path.with_name(path.stem + ".zh-CN.md")
            if companion.is_file():
                current = companion.read_text(encoding="utf-8-sig")
                body = current.split("\n", 1)[1]
                switch = (
                    f"[English]({path.name}) | "
                    f"[\u7b80\u4f53\u4e2d\u6587]({companion.name})"
                )
                companion.write_text(
                    switch + "\n" + localize_links(body, companion),
                    encoding="utf-8", newline="\n",
                )
        return
    only = sys.argv[1:] if len(sys.argv) > 1 else []
    for index, path in enumerate(files, 1):
        if only and path.relative_to(ROOT).as_posix() not in only:
            continue
        source = original(path)
        companion = path.with_name(path.stem + ".zh-CN.md")
        print(f"{index}/{len(files)} {path.relative_to(ROOT)}", flush=True)
        if HAN.search(source):
            english = translate_document(source, "zh_en")
            chinese = source
        else:
            english = source
            chinese = translate_document(source, "en_zh")
        switch = (
            f"[English]({path.name}) | "
            f"[\u7b80\u4f53\u4e2d\u6587]({companion.name})\n\n"
        )
        path.write_text(switch + english.lstrip(), encoding="utf-8", newline="\n")
        companion.write_text(switch + chinese.lstrip(), encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
