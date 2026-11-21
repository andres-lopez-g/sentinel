"""Check local Markdown links and JSON examples in the planning repository."""

import json
from pathlib import Path
import re
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
errors = []
documents = sorted(ROOT.glob("*.md")) + sorted((ROOT / "docs").rglob("*.md"))
for document in documents:
    content = document.read_text(encoding="utf-8")
    for target in re.findall(r"\[[^\]]+\]\(([^)]+)\)", content):
        url = urlsplit(target)
        if url.scheme or not url.path:
            continue
        if not (document.parent / unquote(url.path)).is_file():
            errors.append(f"{document.name}: missing {url.path}")
    for example in re.findall(r"^```json\n(.*?)\n```$", content, re.M | re.S):
        try:
            json.loads(example)
        except ValueError as error:
            errors.append(f"{document.name}: {error}")

if errors:
    raise SystemExit("\n".join(errors))
print(f"Checked local links and JSON examples in {len(documents)} documents.")
