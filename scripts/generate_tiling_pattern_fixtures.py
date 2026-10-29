"""Author PDF tiling patterns and independent Ghostscript/Poppler rasters.

Ghostscript uses arithmetic device CMYK conversion (-dUseFastColor=true),
matching the documented fallback in the importer when no ICC profile exists.
Poppler supplies alpha: Ghostscript 10.02.1 applies the outer alpha twice.
References use 288 dpi so one-pixel antialiasing fringes remain small relative
to the independently checked pattern geometry.
"""
from pathlib import Path
import argparse
import subprocess
import zlib
from generate_nested_stitching_fixture import pdf_bytes, stream

CASES = ['colored', 'matrix', 'ctm-before', 'ctm-after', 'uncolored-rgb',
         'uncolored-cmyk', 'stroke', 'text', 'mask', 'form', 'nested',
         'bbox-offset', 'bbox-clip', 'negative-step', 'overlap', 'alpha',
         'rotate', 'user-unit', 'cell-image', 'cell-text', 'uncolored-mask', 'state-reuse', 'fill-stroke', 'form-alpha', 'cell-alpha', 'flate']


def fixture(kind):
    resources = (b'<< /Pattern << /P 5 0 R /N 9 0 R >> '
                 b'/ColorSpace << /C [/Pattern /DeviceRGB] /K [/Pattern /DeviceCMYK] >> '
                 b'/Font << /F 6 0 R >> /XObject << /M 7 0 R /Fm 8 0 R /I 11 0 R >> '
                 b'/ExtGState << /A 10 0 R >> >>')
    cell_resources = b'<< >>'
    form_resources = resources.replace(b'/Fm 8 0 R ', b'')
    bbox, step, matrix = b'[0 0 8 8]', b'12 /YStep 12', b'[1 0 0 1 0 0]'
    cell = b'1 0 0 rg 0 0 4 8 re f 0 0 1 rg 4 0 4 4 re f'
    paint_type = 2 if kind.startswith('uncolored') else 1
    select = b'/Pattern cs /P scn '
    paint = b'4 4 88 56 re f'
    prefix, suffix = b'', b''
    page_attrs = b''
    if kind == 'state-reuse':
        paint = b'4 4 40 56 re f q 0 .7 0 rg 44 4 8 56 re f Q 52 4 40 56 re f'
    elif kind == 'fill-stroke':
        select += b'/Pattern CS /N SCN '
        paint = b'6 w 8 8 80 48 re B'
    elif kind == 'matrix':
        matrix = b'[1 .25 -.5 1 3 5]'
    elif kind == 'ctm-before':
        prefix = b'1.5 0 0 1.5 3 5 cm '
        paint = b'0 0 56 34 re f'
    elif kind == 'ctm-after':
        select += b'1.5 0 0 1.5 3 5 cm '
        paint = b'0 0 56 34 re f'
    elif kind == 'uncolored-rgb':
        select = b'/C cs .2 .7 .3 /P scn '
        cell = b'0 0 5 8 re f'
    elif kind == 'uncolored-cmyk':
        select = b'/K cs 0 0 0 1 /P scn '
        cell = b'0 0 5 8 re f'
    elif kind == 'uncolored-mask':
        select = b'/C cs .2 .7 .3 /P scn '
        cell = b'8 0 0 8 0 0 cm /M Do'
    elif kind == 'stroke':
        select = b'/Pattern CS /P SCN '
        paint = b'8 w 8 12 m 88 12 l 88 52 l 8 52 l S'
    elif kind == 'text':
        paint = b'BT /F 54 Tf 1 0 0 1 12 12 Tm (HI) Tj ET'
    elif kind == 'mask':
        paint = b'q 88 0 0 56 4 4 cm /M Do Q'
    elif kind in ('form', 'form-alpha'):
        select = b''
        if kind == 'form-alpha':
            prefix = b'/A gs '
        paint = b'q 1 0 0 1 6 8 cm /Fm Do Q'
    elif kind == 'nested':
        cell = b'/Pattern cs /N scn 0 0 8 8 re f'
    elif kind == 'bbox-offset':
        bbox = b'[2 3 10 11]'
        cell = b'1 0 0 rg 2 3 4 8 re f 0 0 1 rg 6 3 4 4 re f'
    elif kind == 'bbox-clip':
        cell = b'1 0 0 rg -4 -4 20 20 re f'
    elif kind == 'negative-step':
        step = b'-12 /YStep -12'
    elif kind == 'overlap':
        step = b'4 /YStep 4'
        cell = b'0 0 0 rg 0 0 6 6 re f'
    elif kind == 'alpha':
        prefix = b'/A gs '
    elif kind == 'rotate':
        page_attrs = b'/Rotate 90 '
    elif kind == 'user-unit':
        page_attrs = b'/UserUnit 2 '
    elif kind == 'cell-image':
        cell = b'q 8 0 0 8 0 0 cm /I Do Q'
    elif kind == 'cell-text':
        bbox, step = b'[0 0 32 32]', b'36 /YStep 36'
        cell = b'0 0 1 rg BT /F 30 Tf 1 0 0 1 0 1 Tm (HI) Tj ET'
    if kind == 'cell-alpha':
        cell_resources = b'<< /ExtGState << /A 10 0 R >> >>'
        cell = b'/A gs '+cell
    elif kind == 'nested':
        cell_resources = b'<< /Pattern << /N 9 0 R >> >>'
    elif kind == 'uncolored-mask':
        cell_resources = b'<< /XObject << /M 7 0 R >> >>'
    elif kind == 'cell-image':
        cell_resources = b'<< /XObject << /I 11 0 R >> >>'
    elif kind == 'cell-text':
        cell_resources = b'<< /Font << /F 6 0 R >> >>'
    cell_filter = b''
    if kind == 'flate':
        cell_filter = b' /Filter /FlateDecode'
        cell = zlib.compress(cell)
    font = b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'
    font_objects = []
    if kind in ('text', 'cell-text'):
        data = (Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures/dejavu-sans.ttf').read_bytes()
        font = (b'<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans /Encoding /WinAnsiEncoding '
                b'/FontDescriptor 12 0 R /FirstChar 72 /LastChar 73 /Widths [752 295] >>')
        font_objects = [
            b'<< /Type /FontDescriptor /FontName /DejaVuSans /Flags 32 /FontBBox [-1021 -463 1794 1232] '
            b'/ItalicAngle 0 /Ascent 928 /Descent -236 /CapHeight 928 /StemV 80 /FontFile2 13 0 R >>',
            stream(f'/Length1 {len(data)} /Filter /FlateDecode'.encode(), zlib.compress(data))
        ]
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64] '+page_attrs+b'/Resources '+resources+b' /Contents 4 0 R >>',
        stream(b'', prefix+select+paint+suffix),
        stream(b'/Type /Pattern /PatternType 1 /PaintType '+str(paint_type).encode()+b' /TilingType 2 /BBox '+bbox+b' /XStep '+step+b' /Matrix '+matrix+b' /Resources '+cell_resources+cell_filter, cell),
        font,
        stream(b'/Type /XObject /Subtype /Image /ImageMask true /Width 4 /Height 4 /BitsPerComponent 1', bytes([0x30,0x30,0xc0,0xc0])),
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 80 48] /Resources '+form_resources, b'/Pattern cs /P scn 0 0 80 48 re f'),
        stream(b'/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 2 /BBox [0 0 4 4] /XStep 4 /YStep 4 /Resources << >>',b'0 .6 0 rg 0 0 2 4 re f'),
        b'<< /ca .5 /CA .5 >>',
        stream(b'/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB',bytes([0,120,230]))
    ]+font_objects)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for kind in CASES:
        name = 'tiling-'+kind
        target = root/(name+'.pdf')
        target.write_bytes(fixture(kind))
        if args.references:
            if kind in ('alpha', 'form-alpha'):
                subprocess.run(['pdftoppm','-r','288','-singlefile','-png',str(target),str(root/name)],check=True)
                continue
            subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-dUseFastColor=true','-sDEVICE=png16m','-r288','-dGraphicsAlphaBits=1',f'-sOutputFile={root/(name+".png")}',str(target)],check=True)
