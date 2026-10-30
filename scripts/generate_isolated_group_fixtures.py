"""Author isolated RGB transparency Forms and original-source GS references."""
from pathlib import Path
import argparse
import subprocess
from generate_nested_stitching_fixture import pdf_bytes, stream

CASES = ['opaque', 'child-alpha', 'group-alpha', 'stroke-alpha', 'parent-blend',
         'child-multiply', 'child-screen', 'mixed-blends', 'bbox', 'clip', 'matrix',
         'reflected', 'rotated', 'user-unit', 'nested', 'inherited-color',
         'image-alpha', 'text', 'gradient', 'tiling', 'pattern-cell', 'reuse',
         'ordinary-form', 'hidden', 'indirect', 'empty', 'text-clip', 'parent-stroke-alpha']


def fixture(name):
    resources = (b'<< /ExtGState << /PG 6 0 R /CH 7 0 R >> /Font << /F1 8 0 R >> '
                 b'/XObject << /Fm 5 0 R /Nested 9 0 R /Im 10 0 R >> '
                 b'/Shading << /S 12 0 R >> /Pattern << /P 14 0 R >> >>')
    group = b' /Group << /Type /Group /S /Transparency /CS /DeviceRGB /I true /K false >>'
    parent = b'/ca 1 /CA 1'
    child = b'/ca .5 /CA .25'
    body = b'1 0 0 rg 8 8 48 40 re f 0 0 1 rg 32 20 48 36 re f'
    content = b'1 1 0 rg 0 0 96 64 re f q /PG gs /Fm Do Q'
    bbox = b'[4 4 88 60]'
    matrix = b'[1 0 0 1 0 0]'
    page_attrs = b''
    catalog_attrs = b''
    form_attrs = b''
    if name == 'child-alpha':
        body = b'/CH gs '+body
    elif name == 'group-alpha':
        parent = b'/ca .5 /CA .1'
    elif name == 'stroke-alpha':
        body = b'/CH gs 8 w 1 0 0 RG 12 12 64 40 re S 0 0 1 rg 32 20 48 36 re f'
    elif name == 'parent-stroke-alpha':
        parent = b'/ca .6 /CA .1'
        body = b'8 w 1 0 0 RG 12 12 64 40 re S 0 0 1 rg 32 20 48 36 re f'
    elif name == 'parent-blend':
        parent = b'/ca .6 /BM /Multiply'
        body = b'/CH gs '+body
    elif name in ('child-multiply', 'child-screen', 'mixed-blends'):
        child = b'/ca .7 /BM '+(b'/Screen' if name == 'child-screen' else b'/Multiply')
        body = b'/CH gs '+body
        if name == 'mixed-blends':
            body += b' /PG gs 0 1 0 rg 20 16 24 36 re f'
    elif name == 'bbox':
        bbox = b'[20 16 68 44]'
    elif name == 'clip':
        content = b'1 1 0 rg 0 0 96 64 re f q 20 16 48 28 re W n /PG gs /Fm Do Q'
    elif name == 'matrix':
        matrix = b'[1 .2 .3 .8 -8 0]'
        content = b'1 1 0 rg 0 0 96 64 re f q 1 0 0 1 6 4 cm /PG gs /Fm Do Q'
    elif name == 'reflected':
        matrix = b'[-1 .1 .2 .9 84 0]'
    elif name == 'rotated':
        page_attrs = b' /Rotate 90'
    elif name == 'user-unit':
        page_attrs = b' /UserUnit 2'
    elif name == 'nested':
        parent = b'/ca .6'
        body = b'1 0 0 rg 8 8 48 40 re f /PG gs /Nested Do 0 0 1 rg 32 20 48 36 re f'
    elif name == 'inherited-color':
        body = b'8 8 48 40 re f'
        content = b'1 1 0 rg 0 0 96 64 re f 0 .4 1 rg /PG gs /Fm Do'
    elif name == 'image-alpha':
        parent = b'/ca .6'
        body = b'48 0 0 40 16 12 cm /Im Do'
    elif name == 'text':
        parent = b'/ca .6'
        body = b'/CH gs 1 0 0 rg BT /F1 32 Tf 16 20 Td (A) Tj ET'
    elif name == 'gradient':
        parent = b'/ca .6'
        body = b'/CH gs /S sh'
    elif name == 'tiling':
        parent = b'/ca .6'
        body = b'/Pattern cs /P scn 8 8 72 48 re f'
    elif name == 'pattern-cell':
        body = b'/CH gs '+body
        content = b'1 1 0 rg 0 0 96 64 re f /Pattern cs /P scn 0 0 96 64 re f'
        child = b'/ca .7 /BM /Multiply'
    elif name == 'reuse':
        body = b'/CH gs 8 8 36 40 re f'
        content = b'1 1 0 rg 0 0 96 64 re f q /PG gs 1 0 0 rg /Fm Do 1 0 0 1 40 0 cm 0 0 1 rg /Fm Do Q'
    elif name == 'ordinary-form':
        group = b''
        parent = b'/ca .5'
    elif name == 'hidden':
        catalog_attrs = b' /OCProperties << /OCGs [15 0 R] /D << /BaseState /OFF /OFF [15 0 R] >> >>'
        form_attrs = b' /OC 15 0 R'
        content = b'/Fm Do'
    elif name == 'indirect':
        group = b' /Group 18 0 R'
    elif name == 'empty':
        body = b'q Q'
        content = b'/Fm Do'
    elif name == 'text-clip':
        content = b'1 1 0 rg 0 0 96 64 re f q BT /F1 44 Tf 12 16 Td 7 Tr (A) Tj ET /PG gs /Fm Do Q'
    pattern_body = b'/Fm Do' if name == 'pattern-cell' else b'1 0 0 rg 0 0 6 6 re f /CH gs 0 0 1 rg 3 3 6 6 re f'
    pattern_box = b'[0 0 96 64] /XStep 96 /YStep 64' if name == 'pattern-cell' else b'[0 0 12 12] /XStep 12 /YStep 12'
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R'+catalog_attrs+b' >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64]'+page_attrs+b' /Resources '+resources+b' /Contents 4 0 R >>',
        stream(b'', content),
        stream(b'/Type /XObject /Subtype /Form /BBox '+bbox+b' /Matrix '+matrix+b' /Resources '+resources+group+form_attrs, body),
        b'<< '+parent+b' >>', b'<< '+child+b' >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>',
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 96 64] /Resources '+resources+group,
               b'/CH gs 0 1 0 rg 12 12 36 36 re f 1 0 0 rg 36 12 36 36 re f'),
        stream(b'/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate true /SMask 11 0 R', bytes([255,0,0,0,255,0,0,0,255,255,0,255])),
        stream(b'/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 8 /Interpolate true',bytes([0,85,170,255])),
        b'<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [8 8 80 56] /Extend [true true] /Function 13 0 R >>',
        b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>',
        stream(b'/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 2 /BBox '+pattern_box+b' /Resources '+resources, pattern_body),
        b'<< /Type /OCG /Name (Hidden group) >>', b'true', b'false',
        b'<< /Type /Group /S /Transparency /CS 19 0 R /I 16 0 R /K 17 0 R >>', b'/DeviceRGB',
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures'
    for name in CASES:
        target = root / f'isolated-group-{name}.pdf'
        target.write_bytes(fixture(name))
        if args.references:
            # GS 10.02 ignores isolation for a Form inside a tiling pattern.
            # Poppler follows 11.6.6 here and matches the independent analytic
            # group-alpha/backdrop tests. Both references use the original PDF.
            if name == 'pattern-cell':
                subprocess.run(['pdftoppm', '-r', '144', '-singlefile', '-png',
                                str(target), str(target.with_suffix(''))], check=True)
            else:
                subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-dUseFastColor=true',
                                '-sDEVICE=png16m','-r144','-dGraphicsAlphaBits=1',
                                f'-sOutputFile={target.with_suffix(".png")}',str(target)], check=True)
