"""Validate local documentation without network or third-party packages."""

import json
import re
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = (
    "README.md", "docs/PRODUCT.md", "docs/ARCHITECTURE.md", "docs/PROTOCOL.md",
    "docs/ROADMAP.md", "docs/DECISIONS.md", "docs/CONTRIBUTING.md", "docs/SYNC.md",
    "docs/MEDIA.md", "docs/SECURITY.md", "docs/TESTING.md", "docs/LICENSING.md",
)


def main() -> int:
    errors = []
    for name in REQUIRED:
        if not (ROOT / name).is_file():
            errors.append(f"Missing document: {name}")
    derived = {"target", "build", ".dart_tool", ".gradle", "ephemeral", ".ci-cache", "dist"}
    docs = sorted(p for p in ROOT.glob("**/*.md")
                  if not derived.intersection(p.relative_to(ROOT).parts))
    json_count = 0
    for path in docs:
        content = path.read_bytes().decode("utf-8")
        if not content.startswith("# "):
            errors.append(f"Missing document title: {path.relative_to(ROOT)}")
        if not content.endswith("\n"):
            errors.append(f"Missing final newline: {path.relative_to(ROOT)}")
        if "\r" in content:
            errors.append(f"Non-LF line ending: {path.relative_to(ROOT)}")
        if len(re.findall(r"^```", content, re.MULTILINE)) % 2:
            errors.append(f"Unbalanced code fences: {path.relative_to(ROOT)}")
        for label, target in re.findall(r"\[([^\]]+)\]\(([^)]+)\)", content):
            if re.match(r"^[a-z]+://", target) or target.startswith("#"):
                continue
            relative = unquote(target.split("#", 1)[0])
            if not (path.parent / relative).exists():
                errors.append(f"Broken link in {path.relative_to(ROOT)}: {label} -> {target}")
        for block in re.findall(r"```json\n(.*?)\n```", content, re.DOTALL):
            try:
                value = json.loads(block)
                json_count += 1
                if "protocol_version" in value:
                    required = {"protocol_version", "event_id", "type", "room_id", "room_epoch",
                                "sender_id", "sequence", "sent_at_ms", "payload"}
                    if not required <= value.keys() or value["protocol_version"] != 1:
                        errors.append(f"Invalid example envelope: {path.relative_to(ROOT)}")
            except json.JSONDecodeError as error:
                errors.append(f"Invalid JSON in {path.relative_to(ROOT)}: {error}")
        for number, line in enumerate(content.splitlines(), 1):
            if line.rstrip() != line:
                errors.append(f"Trailing whitespace: {path.relative_to(ROOT)}:{number}")
    decisions = ROOT / "docs/DECISIONS.md"
    if decisions.is_file():
        entries = re.split(r"^## ADR-", decisions.read_text(encoding="utf-8"), flags=re.MULTILINE)[1:]
        for entry in entries:
            for section in ("Context", "Decision", "Alternatives", "Consequences"):
                if f"**{section}:**" not in entry:
                    errors.append(f"ADR-{entry.splitlines()[0]} missing {section}")
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Documentation OK: {len(docs)} Markdown documents, {json_count} JSON examples, local links checked.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
