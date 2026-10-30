"""Author normal-appearance PDFs and render original-source Ghostscript references.

All vector/image content is repository-authored. The text fixture embeds an AP
subset of the existing licensed DejaVu fixture. Generation requires fonttools:
uv run --with fonttools python scripts/generate_annotation_appearance_fixtures.py --references
"""
from pathlib import Path
import argparse
import subprocess

from generate_nested_stitching_fixture import pdf_bytes, stream

CASES = ["basic", "bbox", "matrix", "reflected", "rotate90", "rotate180", "rotate270",
         "crop", "user-unit", "states", "indirect", "inherited", "shared", "empty",
         "hidden", "link", "text", "image", "isolated", "alpha", "widget", "free-text",
         "annotation-alpha", "no-rotate"]


def fixture(name):
    resources = b'<< /Font << /F1 7 0 R >> /ExtGState << /G 8 0 R >> /XObject << /Im 9 0 R /Nested 10 0 R >> >>'
    body = b'1 0 0 rg 0 0 40 20 re f 0 0 1 rg 20 0 20 10 re f'
    bbox = b'[0 0 40 20]'
    matrix = b'[1 0 0 1 0 0]'
    page = b''
    rect = b'[16 12 80 44]'
    normal = b'5 0 R'
    ap = b'<< /N '+normal+b' >>'
    state = b''
    subtype = b'/Stamp'
    flags = b'4'
    group = b''
    annots = b'[6 0 R]'
    if name == 'bbox':
        bbox = b'[5 4 35 16]'
    elif name == 'matrix':
        matrix = b'[1 .2 .4 .8 -3 7]'
    elif name == 'reflected':
        matrix = b'[-1 0 0 1 40 0]'
    elif name.startswith('rotate'):
        page = b' /Rotate '+name[6:].encode()
    elif name == 'crop':
        page = b' /CropBox [8 4 88 60]'
    elif name == 'user-unit':
        page = b' /UserUnit 2'
    elif name == 'states':
        ap = b'<< /N << /On 5 0 R /Off 10 0 R >> >>'
        state = b' /AS /On'
    elif name == 'indirect':
        ap = b'15 0 R'
        state = b' /AS 11 0 R'
        bbox = b'12 0 R'
        matrix = b'13 0 R'
    elif name == 'shared':
        annots = b'[6 0 R 18 0 R]'
        rect = b'[8 8 40 24]'
    elif name == 'empty':
        body = b'q Q'
    elif name == 'hidden':
        flags = b'6'
    elif name == 'link':
        subtype = b'/Link'
        state = b' /A << /S /URI /URI (https://example.com/appearance) >>'
    elif name == 'text':
        body = b'1 0 0 rg BT /F1 16 Tf 4 4 Td (AP) Tj ET'
    elif name == 'image':
        body = b'40 0 0 20 0 0 cm /Im Do'
    elif name == 'isolated':
        body = b'/G gs '+body
        group = b' /Group << /S /Transparency /CS /DeviceRGB /I true >>'
    elif name == 'alpha':
        body = b'/G gs '+body
    elif name == 'widget':
        subtype = b'/Widget'
        state = b' /FT /Btn /T (button) /V /On /AS /On'
        ap = b'<< /N << /On 5 0 R /Off 10 0 R >> >>'
    elif name == 'free-text':
        subtype = b'/FreeText'
        state = b' /DA (/F1 12 Tf 0 g)'
    elif name == 'annotation-alpha':
        state = b' /CA .5'
    elif name == 'no-rotate':
        page = b' /Rotate 90'
        flags = b'20'
    form_resources = b'' if name == 'inherited' else b' /Resources '+resources
    objects = [
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64] /Resources '+resources+b' /Contents 4 0 R /Annots '+annots+page+b' >>',
        stream(b'', b'1 1 0 rg 0 0 96 64 re f'),
        stream(b'/Type /XObject /Subtype /Form /BBox '+bbox+b' /Matrix '+matrix+form_resources+group, body),
        b'<< /Type /Annot /Subtype '+subtype+b' /Rect '+rect+b' /F '+flags+b' /AP '+ap+state+b' /Contents (custom appearance) /T (author) /NM (normal-appearance) >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        b'<< /ca .5 /CA .5 >>',
        stream(b'/Type /XObject /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Interpolate false', bytes([255,0,0,0,255,0,0,0,255,255,255,255])),
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 40 20]', b'0 1 0 rg 0 0 40 20 re f'),
        b'/On', b'[0 0 40 20]', b'[1 .2 .4 .8 -3 7]',
        b'<< /On 5 0 R /Off 10 0 R >>', b'<< /N 14 0 R >>',
        b'<< >>', b'<< >>',
        b'<< /Type /Annot /Subtype /Stamp /Rect [48 32 88 56] /F 4 /AP << /N 5 0 R >> >>',
    ]
    if name == 'text':
        from io import BytesIO
        from fontTools.ttLib import TTFont
        from fontTools import subset
        font = TTFont(Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures/dejavu-sans.ttf', recalcTimestamp=False)
        cmap = font.getBestCmap()
        widths = [round(font['hmtx'][cmap[code]][0] * 1000 / font['head'].unitsPerEm) for code in range(65, 81)]
        options = subset.Options()
        options.recalc_timestamp = False
        subsetter = subset.Subsetter(options)
        subsetter.populate(text='AP')
        subsetter.subset(font)
        output = BytesIO()
        font.save(output)
        program = output.getvalue()
        objects[6] = (b'<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans /Encoding /WinAnsiEncoding '
                      b'/FirstChar 65 /LastChar 80 /Widths ['+' '.join(map(str,widths)).encode()+b'] /FontDescriptor 20 0 R >>')
        objects.extend([
            stream(f'/Length1 {len(program)}'.encode(),program),
            b'<< /Type /FontDescriptor /FontName /DejaVuSans /Flags 32 /FontBBox [-1021 -463 1793 1232] '
            b'/Ascent 928 /Descent -236 /CapHeight 700 /ItalicAngle 0 /StemV 80 /FontFile2 19 0 R >>',
        ])
    return pdf_bytes(objects)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    target = Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures'
    for name in CASES:
        path = target / f'annotation-ap-{name}.pdf'
        path.write_bytes(fixture(name))
        if args.references:
            subprocess.run(['gs', '-q', '-dSAFER', '-dBATCH', '-dNOPAUSE', '-dUseCropBox',
                            '-dPrinted=false', '-sDEVICE=png16m', '-r144', '-dGraphicsAlphaBits=1',
                            f'-sOutputFile={path.with_suffix(".png")}', str(path)], check=True)
