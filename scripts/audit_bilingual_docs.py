"""Audit language, links, and Markdown shape of bilingual project documentation."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HAN = re.compile(r"[\u3400-\u9fff]")
WORD = re.compile(r"(?<![A-Za-z0-9_])[A-Za-z][A-Za-z0-9_-]*(?![A-Za-z0-9_])")
LINK = re.compile(r"\[([^]]*)\]\(([^)]+)\)")
EXCEPTIONS = {
    "AICore", "AI", "API", "CPU", "GPU", "NPU", "OS", "IPC", "HTTP", "HTTPS",
    "JSON", "ONNX", "LLM", "SLM", "SQL", "SQLite", "SSE", "REST", "CLI", "UI",
    "UX", "CI", "CD", "PID", "UID", "SID", "ACL", "GUID", "SHA-256", "MSVC",
    "DXGI", "DLL", "EP", "FP32", "QDQ", "F32", "AMD", "VitisAI", "DirectML",
    "WinML", "Ollama", "Rust", "Python", "Windows", "Linux", "WSL", "Cargo",
    "Clippy", "PowerShell", "Bash", "Git", "GitHub", "TAR", "DEB", "MSI", "NVIDIA",
    "PASS", "FAIL", "SKIPPED", "NOT", "VERIFIED", "PARTIAL", "RESTORED",
    "NoOp", "NotSupported", "Unsupported", "Unavailable", "SafeMode", "AdaptiveLoop",
    "PureRustRuntime", "ModelRuntimeBinder", "RuntimeBackedProvider", "IIntentAgent",
    "IntentProposal", "IntentAgentCoordinator", "PythonOnnxSessionFactory",
    "CPUExecutionProvider", "DmlExecutionProvider", "VitisAIExecutionProvider",
    "MatMul", "Softmax", "PnP", "LLMs", "ADR", "E2E", "MSRV", "WAL",
    "L0", "L1", "L2", "V2", "V3", "V4", "V5", "V3-2", "V3.2", "p99", "ms",
    "S-1-5-7", "llama3", "bash", "cargo", "git", "sh", "ps1", "rs", "md",
    "README", "destination", "Apache", "License", "NOTICE", "LICENSE", "ID", "IDs",
    "UTC", "RSS", "SDK", "TOML", "Unix", "PIDs", "APIs",
    "Ed25519", "Radeon", "Ack", "MVP", "DTO", "RAM", "ML", "PC",
    "Intel", "Tauri", "Electron", "Liskov", "CLOSED", "CONDITIONAL",
    "YES", "NO", "SCM", "MVP", "Zig", "v1", "v2", "nextest",
    "A", "B", "C", "D", "E", "F",
    "Ubuntu", "UPDATE", "Debian", "systemd", "RPM", "AVAILABLE", "RUN", "EXE",
    "APPLICABLE", "Codex", "DML", "FIFO", "pip", "ZIP", "PATH", "cc-rs",
    "Tokio", "sysfs", "DRM", "ACPI", "AC", "SHA256",
    "Service", "Named", "Pipe", "Dashboard", "WiX", "Toolset", "Authenticode",
    "Web", "MB", "v3",
}


def body_lines(text: str) -> list[tuple[int, str]]:
    """Keep prose and link labels, but ignore code and link destinations."""
    result = []
    fenced = False
    for number, line in enumerate(text.splitlines()[1:], 2):
        if line.lstrip().startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        line = re.sub(r"`[^`]*`", "", line)
        line = LINK.sub(lambda match: match.group(1), line)
        line = re.sub(r"https?://\S+", "", line)
        line = re.sub(r"(?<!\w)(?:[A-Za-z0-9_.-]+[/\\])+[A-Za-z0-9_.-]+", "", line)
        line = re.sub(r"(?<!\w)[A-Za-z0-9_.-]+\.(?:rs|md|toml|json|sh|ps1|py|wxs|log|txt|dll|exe|onnx|argosmodel)\b", "", line)
        line = re.sub(r"(?<!\w)[A-Za-z_][A-Za-z0-9_]*\.[A-Za-z_][A-Za-z0-9_.]*", "", line)
        line = re.sub(r"--[A-Za-z][\w-]*", "", line)
        line = re.sub(r"(?<!\w)(?:cargo|bash|python|git|rustc)\s+[^|，。；]*", "", line)
        result.append((number, line))
    return result


def shape(text: str) -> tuple[tuple[int, ...], tuple[str, ...], tuple[int, ...], tuple[str, ...]]:
    lines = text.splitlines()
    headings = tuple(len(match.group(1)) for line in lines if (match := re.match(r"^(#{1,6})\s", line)))
    fences = tuple(line.strip() for line in lines if line.lstrip().startswith("```"))
    table_rows = tuple(line.count("|") for line in lines if re.match(r"^\s*\|", line))
    links = tuple(target.replace(".zh-CN.md", ".md") for _, target in LINK.findall(text))
    return headings, fences, table_rows, links


def main() -> None:
    listed = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT,
    )
    relative_paths = [
        Path(raw.decode("utf-8"))
        for raw in listed.split(b"\0")
        if raw
    ]
    files = sorted(
        ROOT / path
        for path in relative_paths
        if path.suffix.lower() == ".md"
        and not path.name.endswith(".zh-CN.md")
        and not path.parts[0].lower() in {"env", "target"}
    )
    issues = []
    for path in files:
        relative = path.relative_to(ROOT).as_posix()
        companion = path.with_name(path.stem + ".zh-CN.md")
        if not companion.exists():
            issues.append(f"{relative}: missing Chinese companion")
            continue
        english = path.read_text(encoding="utf-8-sig")
        chinese = companion.read_text(encoding="utf-8-sig")
        en_switch = f"[English]({path.name}) | [Chinese]({companion.name})"
        zh_switch = f"[\u82f1\u6587]({path.name}) | [\u4e2d\u6587]({companion.name})"
        if english.splitlines()[0] != en_switch or chinese.splitlines()[0] != zh_switch:
            issues.append(f"{relative}: language switch missing or wrong")
        if shape(english) != shape(chinese):
            issues.append(f"{relative}: EN/ZH structure {shape(english)} != {shape(chinese)}")
        for match in HAN.finditer(english):
            line = english.count("\n", 0, match.start()) + 1
            issues.append(f"{relative}:{line}: Chinese in English document")
        for line_number, line in body_lines(chinese):
            for match in WORD.finditer(line):
                word = match.group()
                code_identifier = "_" in word or bool(re.search(r"[a-z][A-Z]", word))
                requirement_id = bool(re.fullmatch(r"(?:R|FR|D)-\d+", word))
                if word not in EXCEPTIONS and not code_identifier and not requirement_id and not re.fullmatch(r"[A-Z][0-9]+", word):
                    issues.append(f"{companion.relative_to(ROOT).as_posix()}:{line_number}: English word {word}")
        for document in (path, companion):
            content = document.read_text(encoding="utf-8-sig")
            for _, target in LINK.findall(content):
                if target.startswith(("http://", "https://", "#", "mailto:")):
                    continue
                local = target.split("#", 1)[0]
                if local and not (document.parent / local).exists():
                    issues.append(f"{document.relative_to(ROOT).as_posix()}: missing link {target}")
    print(f"Checked {len(files)} bilingual document pairs")
    for issue in issues:
        print(issue)
    print(f"Issues: {len(issues)}")
    if issues:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
