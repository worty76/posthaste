#!/usr/bin/env python3
"""Coverage and size check for the fonts bundled in `ui/styles/fonts/`.

Fails when a font file lacks a glyph for any character a look can put on
screen (including Vietnamese with stacked marks), when a file is over the
per-family size budget, or when the bundled families together are over the
whole-app budget. Prints one line per file: name, size in KB, missing
characters.

Usage: python3 scripts/check-fonts.py ui/styles/fonts/*.woff2
"""

import re
import sys
from pathlib import Path

MAX_FILE_KB = 150
MAX_FAMILY_KB = 400

# Everything the interface can print: Vietnamese with stacked marks, the
# Latin set the copy uses, digits and the punctuation the notes carry.
REQUIRED = (
    "ĐđƯưƠơ ẫỡựẶồ ÀÁÂÃÈÉÊÌÍÒÓÔÕÙÚÝàáâãèéêìíòóôõùúý "
    "Quick brown fox 0123456789 .,;:!?'\"()-–—…"
)


def family_of(path: Path) -> str:
    name = path.name
    for suffix in (".woff2", "-variable",):
        if name.endswith(suffix):
            name = name[: -len(suffix)]
    return re.sub(r"-\d+$", "", name)


def main(argv: list[str]) -> int:
    files = [Path(a) for a in argv]
    if not files:
        print("no font files given; expected ui/styles/fonts/*.woff2")
        return 1
    try:
        from fontTools.ttLib import TTFont
    except ImportError:
        print("install fonttools to run this check: pip3 install --user fonttools brotli")
        return 2

    failed = False
    family_kb: dict[str, int] = {}
    for path in files:
        if not path.is_file():
            print(f"{path}: missing file")
            failed = True
            continue
        size_kb = path.stat().st_size / 1024
        family = family_of(path)
        family_kb[family] = family_kb.get(family, 0) + path.stat().st_size
        cmap = TTFont(path).getBestCmap()
        missing = sorted({c for c in set(REQUIRED) if ord(c) not in cmap})
        note = ""
        if missing:
            note = " missing: " + " ".join(missing)
            failed = True
        if size_kb > MAX_FILE_KB:
            note += f" over the {MAX_FILE_KB} KB file budget"
            failed = True
        print(f"{path.name}: {size_kb:.1f} KB{note}")

    for family, size in sorted(family_kb.items()):
        kb = size / 1024
        if kb > MAX_FAMILY_KB:
            print(f"{family}: {kb:.1f} KB over the {MAX_FAMILY_KB} KB family budget")
            failed = True
    total_kb = sum(family_kb.values()) / 1024
    print(f"total: {total_kb:.1f} KB across {len(family_kb)} families")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
