"""Author empty ActualText fixtures and independent original-PDF rasters.

uv run --with fonttools python scripts/generate_empty_actual_text_fixtures.py
TrueType uses the licensed DejaVu fixture; CFF outlines are repository-authored.
Solid glyph references use Ghostscript. Dashed text uses Poppler, cross-checked
with MuPDF: the installed Ghostscript instead produced a solid glyph stroke.
Stroke references use 576 dpi to reduce text grid-fitting differences.
"""
from pathlib import Path
import subprocess
from generate_short_actual_text_fixtures import truetype
from generate_cff_pdf_fixtures import font, pdf

def main():
    fixtures = Path(__file__).resolve().parents[1] / "crates/ofdrw/tests/fixtures"
    cases = {
        "truetype": truetype(fixtures, b"1 0 0 1 0 0", empty=True),
        "affine": truetype(fixtures, b"0.8 0.15 -0.2 0.9 20 0", empty=True),
        "reflected": truetype(fixtures, b"-1 0.1 0.15 1 100 0", empty=True),
    }
    cases["truetype-stroke"] = truetype(fixtures, b"1 0 0 1 0 0", empty=True, paint=b"2 Tr 0.7 w 1 0 0 rg 0 0 1 RG")
    cases["truetype-dashed-stroke"] = truetype(fixtures, b"1 0 0 1 0 0", empty=True, paint=b"2 Tr 0.7 w [2 1] 0.25 d 1 J 2 j 1 0 0 rg 0 0 1 RG")
    cases["truetype-matrix-stroke"] = truetype(fixtures, b"1 0 0 1 0 0", empty=True, paint=b"2 Tr 0.7 w 1 0.2 0.25 0.8 0 0 Tm 1 0 0 rg 0 0 1 RG")
    opentype, program = font()
    cases["cff-opentype"] = pdf(opentype, simple=True, actual_text="")
    cases["cff-stroke"] = pdf(program, raw=True, simple=True, actual_text="", paint=b"2 Tr 0.7 w 1 0 0 rg 0 0 1 RG")
    cases["cff-dashed-stroke"] = pdf(program, raw=True, simple=True, actual_text="", paint=b"2 Tr 0.7 w [2 1] 0.25 d 1 J 2 j 1 0 0 rg 0 0 1 RG")
    cases["cff-matrix-stroke"] = pdf(program, raw=True, simple=True, actual_text="", paint=b"2 Tr 0.7 w 1 0.2 0.25 0.8 0 0 Tm 1 0 0 rg 0 0 1 RG")
    cases["cff-pattern"] = pdf(program, raw=True, simple=True, actual_text="", pattern=True, paint=b"/Pattern cs /P scn")
    cases["cff-pattern-matrix-stroke"] = pdf(program, raw=True, simple=True, actual_text="", pattern=True, paint=b"2 Tr 1 w 1 0.2 0.25 0.8 0 0 Tm /Pattern cs /P scn /Pattern CS /P SCN")
    cases["cff"] = pdf(program, raw=True, simple=True, actual_text="")
    _, program = font(font_matrix=[.001, .0002, 0, .001, .04, .03])
    cases["cff-matrix"] = pdf(program, raw=True, simple=True, actual_text="")
    _, program = font(cid=True, fd_matrices=[None, [1.2, .1, 0, .9, 0, 0]], fd_select=[0, 0, 1])
    cases["cid-cff-matrix"] = pdf(program, raw=True, cid=True, actual_text="")
    for name, content in cases.items():
        path = fixtures / f"actual-text-empty-{name}.pdf"
        path.write_bytes(content)
        if "dashed" in name:
            subprocess.run([
                "pdftoppm", "-r", "576", "-png", "-singlefile", str(path),
                str(path.with_suffix("")),
            ], check=True)
        else:
            subprocess.run([
                "gs", "-q", "-dSAFER", "-dBATCH", "-dNOPAUSE", "-sDEVICE=png16m",
                f"-r{576 if name.endswith('stroke') else 144}",
                "-dTextAlphaBits=4", "-dGraphicsAlphaBits=4",
                f"-sOutputFile={path.with_suffix('.png')}", str(path),
            ], check=True)
        print(path.name, len(content))


if __name__ == "__main__":
    main()
