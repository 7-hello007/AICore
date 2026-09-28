"""Project-local, deterministic Chinese/English translation for documentation."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "env" / "translation-packages"))

import ctranslate2  # noqa: E402
import sentencepiece as spm  # noqa: E402


TECHNICAL = re.compile(
    r"(`[^`\n]+`|\]\([^\n)]+\)|https?://\S+|"
    r"(?:/[A-Za-z0-9_.-]+){2,}|"
    r"\b[A-Za-z0-9_.-]+\.(?:rs|toml|json|sh|py|ps1|dll|exe|db)\b|"
    r"\b(?:Success|Failed|Failure|Skipped|NoOp|NotSupported|Unsupported|"
    r"PermissionDenied|AllSucceeded|AllFailed|PartiallySucceeded|Expired|Cancelled)\b|"
    r"\b(?:CPU|GPU|NPU|IPC|API|AI|ONNX|LLM|SLM|JSON|HTTP|SSE|SQL|" 
    r"Core|Adapter|Runtime|Provider|Windows|Linux|Ollama|DirectML|VitisAI|"
    r"[A-Z][A-Za-z0-9_]*[A-Z][A-Za-z0-9_]*|[A-Za-z0-9]+_[A-Za-z0-9_]+)\b)"
)
HAN = re.compile(r"[\u3400-\u9fff]")
MODELS: dict[str, tuple[object, object]] = {}


def model(direction: str) -> tuple[object, object]:
    if direction not in MODELS:
        folder = ROOT / "env" / "translation-models" / f"translate-{direction}-1_9"
        tokenizer = spm.SentencePieceProcessor(model_file=str(folder / "sentencepiece.model"))
        translator = ctranslate2.Translator(
            str(folder / "model"), device="cpu", compute_type="int8", inter_threads=1,
            intra_threads=4,
        )
        MODELS[direction] = tokenizer, translator
    return MODELS[direction]


def translate_plain(source: str, direction: str) -> str:
    if direction == "zh_en" and not HAN.search(source):
        return source
    if direction == "en_zh" and not re.search(r"[A-Za-z]{2,}", source):
        return source
    left = re.match(r"\s*", source).group()
    right = re.search(r"\s*$", source).group()
    text = source[len(left) : len(source) - len(right) if right else len(source)]
    if not text:
        return source
    tokenizer, translator = model(direction)
    tokens = tokenizer.encode(text, out_type=str)
    if not tokens:
        return source
    result = translator.translate_batch(
        [tokens], replace_unknowns=True, beam_size=4, length_penalty=0.2
    )[0].hypotheses[0]
    translated = tokenizer.decode_pieces(result).replace("\u2581", " ").strip()
    translated = translated.replace("Runtume", "Runtime")
    return left + translated + right


def translate(source: str, direction: str) -> str:
    saved: dict[str, str] = {}

    def replace(match: re.Match[str]) -> str:
        key = f"XCODE{len(saved):03d}X"
        saved[key] = match.group()
        return key

    masked = TECHNICAL.sub(replace, source)
    output = translate_plain(masked, direction)
    if any(key not in output for key in saved):
        # Preserve literals even when a translation model drops a placeholder.
        output = "".join(
            part if part in saved else translate_plain(part, direction)
            for part in re.split(r"(XCODE\d{3}X)", masked)
        )
    for key, value in saved.items():
        output = output.replace(key, value)
    return output
