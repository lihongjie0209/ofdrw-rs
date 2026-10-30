"""Generate an ActualText fixture with repository-authored CFF outlines.

Requires fonttools. Glyph construction and licensing are documented in
scripts/generate_cff_pdf_fixtures.py; no system font is used.
"""
from pathlib import Path

from generate_cff_pdf_fixtures import font, pdf

if __name__ == "__main__":
    _, program = font()
    target = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures/actual-text-cff.pdf"
    target.write_bytes(pdf(program, raw=True, simple=True, actual_text="中文"))
