"""Author short ActualText sequences and independent original-PDF rasters.

The TrueType font is a subset of the licensed DejaVu Serif fixture; all name
records (including notices) are retained. CFF outlines are repository-authored
by generate_cff_pdf_fixtures.py. No system font or Rust rendering is used.

uv run --with fonttools python scripts/generate_short_actual_text_fixtures.py --references
"""
import argparse
from io import BytesIO
from pathlib import Path
import subprocess

from fontTools import subset
from fontTools.ttLib import TTFont

from generate_cff_pdf_fixtures import font, pdf as cff_pdf
from generate_pdf_glyph_fixtures import objects
from generate_nested_stitching_fixture import pdf_bytes, stream


def truetype(fixtures, matrix):
    program = TTFont(fixtures / "dejavu-serif.ttf", recalcTimestamp=False)
    options = subset.Options()
    options.recalc_timestamp = False
    options.glyph_names = True
    options.notdef_outline = True
    options.name_IDs = ["*"]
    options.name_languages = ["*"]
    subsetter = subset.Subsetter(options)
    subsetter.populate(text="A\ufb01")
    subsetter.subset(program)
    output = BytesIO()
    program.save(output)
    parts = objects(output.getvalue(), simple=True)
    content = parts[3].split(b"stream\n", 1)[1].rsplit(b"endstream", 1)[0].rstrip(b"\n")
    parts[3] = stream(b"", b"q " + matrix + b" cm /Span << /ActualText (X) >> BDC\n" + content + b"\nEMC Q")
    return pdf_bytes(parts)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--references", action="store_true")
    args = parser.parse_args()
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    cases = {
        "truetype": truetype(fixtures, b"1 0 0 1 0 0"),
        "affine": truetype(fixtures, b"0.8 0.15 -0.2 0.9 20 0"),
        "reflected": truetype(fixtures, b"-1 0.1 0.15 1 100 0"),
    }
    _, program = font()
    cases["cff"] = cff_pdf(program, raw=True, simple=True, actual_text="X")
    _, program = font(font_matrix=[.001, .0002, 0, .001, .04, .03])
    cases["cff-matrix"] = cff_pdf(program, raw=True, simple=True, actual_text="X")
    _, program = font(cid=True, fd_matrices=[None, [1.2, .1, 0, .9, 0, 0]], fd_select=[0, 0, 1])
    cases["cid-cff-matrix"] = cff_pdf(program, raw=True, cid=True, actual_text="X")
    for name, content in cases.items():
        path = fixtures / f"actual-text-short-{name}.pdf"
        path.write_bytes(content)
        if args.references:
            subprocess.run(["gs", "-q", "-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=png16m", "-r144", "-dTextAlphaBits=4", "-dGraphicsAlphaBits=4", f"-sOutputFile={path.with_suffix('.png')}", str(path)], check=True)
        print(path.name, len(content))


if __name__ == "__main__":
    main()
