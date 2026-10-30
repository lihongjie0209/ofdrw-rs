"""Author graphics-state soft masks with independent original-PDF rasters."""
from pathlib import Path
import argparse
import subprocess
import zlib
from generate_nested_stitching_fixture import pdf_bytes, stream

CASES = ['alpha', 'luminosity', 'alpha-invert', 'luminosity-bc', 'mask-matrix',
         'captured-ctm', 'q-restore', 'none', 'repeat', 'group', 'parent-alpha',
         'multiply', 'stroke', 'fill-stroke', 'text', 'gradient',
         'lum-nonisolated', 'indirect', 'image', 'inline-image', 'ordinary-form',
         'shaded-paint', 'pattern', 'pattern-cell-mask', 'sampled', 'calculator',
         'stitching', 'nested-masks', 'resource-scope', 'fill-stroke-alpha',
         'replace', 'rotate', 'user-unit', 'lum-nonisolated-blend', 'text-fill-stroke-alpha', 'text-clip', 'compressed', 'mask-group-oc', 'hidden-mask-content']


def fixture(name):
    resources = (b'<< /ExtGState << /M 6 0 R /CH 8 0 R /Clear 15 0 R /N 22 0 R >> '
                 b'/Font << /F1 9 0 R >> /XObject << /F 11 0 R /Im 19 0 R >> '
                 b'/Shading << /S 13 0 R >> /Pattern << /P 21 0 R >> /Properties << /H 25 0 R >> >>')
    lum = name.startswith('lum')
    mask = b'<< /Type /Mask /S '+(b'/Luminosity' if lum else b'/Alpha')+b' /G 5 0 R'
    if name == 'alpha-invert':
        mask += b' /TR 10 0 R'
    elif name in ('luminosity-bc', 'lum-nonisolated', 'lum-nonisolated-blend'):
        mask += b' /BC [0 1 0]'
    if name in ('sampled', 'calculator', 'stitching'):
        mask += b' /TR 10 0 R'
    mask += b' >>'
    group_body = b'/CH gs .8 g 8 8 48 48 re f'
    matrix = b'[1 0 0 1 0 0]'
    attrs = b'/CS /DeviceRGB /I true /K false'
    if name.startswith('lum-nonisolated'):
        attrs = b'/CS /DeviceRGB /I false /K false'
        group_body = b'/CH gs /Clear gs .8 g 8 8 48 48 re f'
    if name == 'mask-matrix':
        matrix = b'[.8 .2 .2 .8 4 0]'
    elif name == 'gradient':
        group_body = b'/S sh'
    elif name == 'indirect':
        attrs = b'/CS 16 0 R /I 17 0 R /K 18 0 R'
    background = b'1 1 0 rg 0 0 96 64 re f '
    content = background+b'/M gs 0 0 1 rg 4 4 88 56 re f'
    if name == 'captured-ctm':
        content = background+b'q .8 0 0 .8 8 4 cm /M gs 1 0 0 1 20 0 cm 0 0 1 rg 0 0 96 64 re f Q'
    elif name == 'q-restore':
        content = background+b'q /M gs 0 0 1 rg 4 4 88 56 re f Q 1 0 0 rg 72 8 16 48 re f'
    elif name == 'none':
        content = background+b'/M gs 0 0 1 rg 4 4 88 56 re f /Clear gs 1 0 0 rg 72 8 16 48 re f'
    elif name == 'repeat':
        content = background+b'/M gs 0 0 1 rg 4 4 88 56 re f 1 0 0 rg 24 16 48 32 re f'
    elif name == 'group':
        content = background+b'/M gs /F Do'
    elif name == 'stroke':
        content = background+b'/M gs 0 0 1 RG 8 w 12 12 64 40 re S'
    elif name == 'fill-stroke':
        content = background+b'/M gs 0 0 1 rg 1 0 0 RG 8 w 12 12 64 40 re B'
    elif name == 'text':
        content = background+b'/M gs 0 0 1 rg BT /F1 40 Tf 12 16 Td (A) Tj ET'
    if name in ('ordinary-form', 'resource-scope'):
        content = background+b'/M gs /F Do'
    elif name == 'image':
        content = background+b'/M gs 64 0 0 48 8 8 cm /Im Do'
    elif name == 'inline-image':
        content = background+b'/M gs 64 0 0 48 8 8 cm BI /W 1 /H 1 /CS /RGB /BPC 8 ID '+bytes([0,0,255])+b' EI'
    elif name == 'shaded-paint':
        content = background+b'/M gs /S sh'
    elif name == 'pattern':
        content = background+b'/M gs /Pattern cs /P scn 0 0 96 64 re f'
    elif name == 'pattern-cell-mask':
        content = background+b'/Pattern cs /P scn 0 0 96 64 re f'
    elif name == 'fill-stroke-alpha':
        content = background+b'/M gs 0 0 1 rg 1 0 0 RG 8 w 12 12 64 40 re B'
    if name == 'replace':
        content = background+b'/M gs /N gs 0 0 1 rg 4 4 88 56 re f'
    if name == 'text-fill-stroke-alpha':
        content = background+b'/M gs 0 0 1 rg 1 0 0 RG 2 w BT /F1 40 Tf 12 16 Td 2 Tr (A) Tj ET'
    if name == 'text-clip':
        content = background+b'/M gs BT /F1 40 Tf 12 16 Td 7 Tr (A) Tj ET /Clear gs 0 0 1 rg 0 0 96 64 re f'
    if name == 'nested-masks':
        group_body = b'/CH gs /N gs .8 g 8 8 48 48 re f'
    group_resources = b' /Resources '+resources
    if name == 'resource-scope':
        group_resources = b''
        group_body = b'BT /F1 40 Tf 12 16 Td (A) Tj ET'
    transfer = b'<< /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 1 >>'
    if name == 'sampled':
        transfer = stream(b'/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [3] /BitsPerSample 8', bytes([0,64,255]))
    elif name == 'calculator':
        transfer = stream(b'/FunctionType 4 /Domain [0 1] /Range [0 1]', b'{ .5 lt { 0 } { 1 } ifelse }')
    elif name == 'stitching':
        transfer = b'<< /FunctionType 3 /Domain [0 1] /Bounds [.5] /Encode [0 1 0 1] /Functions [<< /FunctionType 2 /Domain [0 1] /C0 [.25] /C1 [.25] /N 1 >> << /FunctionType 2 /Domain [0 1] /C0 [.75] /C1 [.75] /N 1 >>] >>'
    form_group = b'' if name in ('ordinary-form', 'resource-scope') else b' /Group << /S /Transparency /CS /DeviceRGB /I true >>'
    form_resources = resources if name != 'resource-scope' else b'<< /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >>'
    pattern_body = b'/M gs 0 0 1 rg 0 0 24 16 re f' if name == 'pattern-cell-mask' else b'0 0 1 rg 0 0 12 16 re f 1 0 0 rg 12 0 12 16 re f'
    parent = b'/ca .6 /CA .3' if name in ('parent-alpha', 'fill-stroke-alpha', 'text-fill-stroke-alpha') else b''
    if name == 'multiply':
        parent = b'/BM /Multiply'
    if name == 'hidden-mask-content':
        group_body = b'/OC /H BDC '+group_body+b' EMC'
    group_filter = b' /Filter /FlateDecode' if name == 'compressed' else b''
    if name == 'compressed':
        group_body = zlib.compress(group_body)
    page_attrs = b' /Rotate 90' if name == 'rotate' else b' /UserUnit 2' if name == 'user-unit' else b''
    child = b'/ca .5 /BM /Screen' if name == 'lum-nonisolated-blend' else b'/ca .5'
    catalog_attrs = b' /OCProperties << /OCGs [25 0 R] /D << /BaseState /OFF /OFF [25 0 R] >> >>' if name in ('mask-group-oc', 'hidden-mask-content') else b''
    group_oc = b' /OC 25 0 R' if name == 'mask-group-oc' else b''
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R'+catalog_attrs+b' >>', b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64]'+page_attrs+b' /Resources '+resources+b' /Contents 4 0 R >>',
        stream(b'',content),
        stream(b'/Type /XObject /Subtype /Form /BBox [8 8 88 56] /Matrix '+matrix+b' /Group << /S /Transparency '+attrs+b' >>'+group_resources+group_filter+group_oc, group_body),
        b'<< /SMask 7 0 R '+parent+b' >>', mask, b'<< '+child+b' >>',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>',
        transfer,
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 96 64]'+form_group+b' /Resources '+form_resources,
               b'1 0 0 rg 8 8 48 40 re f 0 0 1 rg 32 20 48 36 re f'),
        b'null',
        b'<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [8 8 88 56] /Extend [true true] /Function 14 0 R >>',
        b'<< /FunctionType 2 /Domain [0 1] /C0 [0 0 0] /C1 [1 1 1] /N 1 >>',
        b'<< /SMask /None >>', b'/DeviceRGB', b'true', b'false',
        stream(b'/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate true /SMask 20 0 R',bytes([0,0,255])*4),
        stream(b'/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 8 /Interpolate true',bytes([0,85,170,255])),
        stream(b'/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 2 /BBox [0 0 24 16] /XStep 24 /YStep 16 /Resources '+resources,pattern_body),
        b'<< /SMask 23 0 R >>', b'<< /S /Alpha /G 24 0 R >>',
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 96 64] /Group << /S /Transparency /CS /DeviceRGB /I true >> /Resources '+resources,
               b'/CH gs 0 g 0 0 96 64 re f'),
        b'<< /Type /OCG /Name (Hidden mask) >>',
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures'
    for name in CASES:
        path = root / f'soft-mask-{name}.pdf'
        path.write_bytes(fixture(name))
        if args.references:
            if name == 'inline-image':
                # GS 10.02 loses the mask group's fractional alpha on inline
                # images. Poppler matches the analytic 0.5-alpha expectation.
                subprocess.run(['pdftoppm','-r','144','-singlefile','-png',
                                str(path),str(path.with_suffix(''))],check=True)
            else:
                subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-dUseFastColor=true',
                                '-sDEVICE=png16m','-r144','-dGraphicsAlphaBits=1',
                                f'-sOutputFile={path.with_suffix(".png")}',str(path)], check=True)
