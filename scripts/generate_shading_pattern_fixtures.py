"""Author Type-2 pattern PDFs and independent Ghostscript/Poppler references.

Ghostscript 10.02.1 and Poppler ignore alpha in a shading Pattern ExtGState.
For those cases, equivalent references move the Normal-group alpha to the
painting of an explicit isolated Form group, preserving the product of source
and outer opacity. The Form also prevents background/shading alpha from being
applied separately by the reference renderer. Text embeds DejaVu Sans to avoid rasterizer font substitution.
"""
from pathlib import Path
import argparse
import subprocess
import zlib
from generate_nested_stitching_fixture import pdf_bytes, stream
from generate_plane_shading_fixtures import fixture as plane
from generate_mesh_shading_fixtures import mesh
from generate_patch_shading_fixtures import patch_pdf

CASES = ['axial', 'radial', 'affine-axial', 'affine-radial', 'reflect',
         'ctm-before', 'ctm-after', 'background', 'bbox-background',
         'ext-alpha', 'outer-alpha', 'combined-alpha', 'background-alpha',
         'stroke', 'text', 'mask', 'form', 'rotate', 'user-unit', 'state-reuse',
         'indirect', 'nested', 'irrelevant-gstate', 'plane', 'mesh', 'lattice',
         'coons', 'tensor']


def authored_objects(data):
    """Read the uncompressed xref emitted by our own pdf_bytes helper."""
    xref = int(data.rsplit(b'startxref\n', 1)[1].splitlines()[0])
    lines = data[xref:].splitlines()
    assert lines[0] == b'xref'
    first, count = map(int, lines[1].split())
    assert first == 0
    offsets = [int(line.split()[0]) for line in lines[2:2+count]] + [xref]
    result = []
    for index in range(1, count):
        source = data[offsets[index]:offsets[index+1]]
        header = f'{index} 0 obj\n'.encode()
        assert source.startswith(header) and source.endswith(b'\nendobj\n')
        result.append(source[len(header):-len(b'\nendobj\n')])
    return result


def wrapped_shading(kind):
    if kind == 'plane':
        objects = authored_objects(plane('plane-calculator-rgb'))
    elif kind in ('mesh', 'lattice'):
        objects = authored_objects(mesh(kind=5 if kind == 'lattice' else 4))
    else:
        objects = authored_objects(patch_pdf(kind=6 if kind == 'coons' else 7, curved=True))
    pattern_id = len(objects) + 1
    objects[2] = (b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] '
                  + f'/Resources << /Pattern << /P {pattern_id} 0 R >> >> /Contents 4 0 R >>'.encode())
    objects[3] = stream(b'', b'/Pattern cs /P scn 4 4 88 88 re f')
    objects.append(b'<< /Type /Pattern /PatternType 2 /Shading 5 0 R >>')
    return pdf_bytes(objects)


def fixture(kind, equivalent_alpha=False):
    if kind in ('plane', 'mesh', 'lattice', 'coons', 'tensor'):
        return wrapped_shading(kind)
    resources = (b'<< /Pattern << /P 5 0 R /T 12 0 R >> /Font << /F 8 0 R >> '
                 b'/XObject << /M 9 0 R /Fm 10 0 R >> /ExtGState << /A 11 0 R >> >>')
    # /T is only declared for the nested case; other scenes must have no dangling reference.
    if kind != 'nested':
        resources = resources.replace(b' /T 12 0 R', b'')
    matrix = b'[1 0 0 1 0 0]'
    shader = b'/ShadingType 2 /ColorSpace /DeviceRGB /Coords [12 32 84 32] /Function 7 0 R /Extend [true true]'
    prefix = b''
    select = b'/Pattern cs /P scn '
    paint = b'4 4 88 56 re f'
    attrs = b''
    ext = b''
    extra = []
    font = b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'
    if kind in ('radial', 'affine-radial'):
        shader = b'/ShadingType 3 /ColorSpace /DeviceRGB /Coords [48 32 0 48 32 32] /Function 7 0 R /Extend [true true]'
    if kind.startswith('affine'):
        matrix = b'[1 .2 -.3 .8 10 3]'
    elif kind == 'reflect':
        matrix = b'[-1 0 0 1 96 0]'
    elif kind == 'ctm-before':
        prefix = b'1.2 0 0 1.2 3 5 cm '
        paint = b'0 0 72 44 re f'
    elif kind == 'ctm-after':
        select += b'1.2 0 0 1.2 3 5 cm '
        paint = b'0 0 72 44 re f'
    elif kind in ('background', 'bbox-background', 'background-alpha'):
        shader = shader.replace(b'[12 32 84 32]', b'[32 32 64 32]').replace(b'[true true]', b'[false false]')
        shader += b' /Background [.2 .8 .3]'
        if kind == 'bbox-background':
            shader += b' /BBox [20 10 76 54]'
    elif kind == 'stroke':
        select = b'/Pattern CS /P SCN '
        paint = b'8 w 8 12 m 88 12 l 88 52 l 8 52 l S'
    elif kind == 'mask':
        paint = b'q 88 0 0 56 4 4 cm /M Do Q'
    elif kind == 'form':
        select = b''
        paint = b'q 1 0 0 1 6 8 cm /Fm Do Q'
    elif kind == 'rotate':
        attrs = b'/Rotate 90 '
    elif kind == 'user-unit':
        attrs = b'/UserUnit 2 '
    elif kind == 'state-reuse':
        paint = b'4 4 40 56 re f q 0 .7 0 rg 44 4 8 56 re f Q 52 4 40 56 re f'
    elif kind == 'irrelevant-gstate':
        ext = b' /ExtGState << /LW 100 /CA 0 /Font [8 0 R 12] /D [[3 2] 0] >>'
    elif kind == 'indirect':
        matrix = b'12 0 R'
        ext = b' /ExtGState 13 0 R'
        extra = [b'[1 .2 -.3 .8 10 3]', b'<< /ca 1 /RI /RelativeColorimetric >>']
    elif kind == 'nested':
        select = b'/Pattern cs /T scn '
        extra = [stream(b'/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 2 /BBox [0 0 32 32] /XStep 32 /YStep 32 /Resources << /Pattern << /S 5 0 R >> >>',
                        b'/Pattern cs /S scn 0 0 24 24 re f')]
    elif kind == 'text':
        paint = b'BT /F 54 Tf 1 0 0 1 12 12 Tm (HI) Tj ET'
        data = (Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures/dejavu-sans.ttf').read_bytes()
        font = (b'<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans /Encoding /WinAnsiEncoding '
                b'/FontDescriptor 12 0 R /FirstChar 72 /LastChar 73 /Widths [752 295] >>')
        extra = [b'<< /Type /FontDescriptor /FontName /DejaVuSans /Flags 32 /FontBBox [-1021 -463 1794 1232] '
                 b'/ItalicAngle 0 /Ascent 928 /Descent -236 /CapHeight 928 /StemV 80 /FontFile2 13 0 R >>',
                 stream(f'/Length1 {len(data)} /Filter /FlateDecode'.encode(), zlib.compress(data))]
    if kind in ('ext-alpha', 'combined-alpha', 'background-alpha'):
        ext = b' /ExtGState << /ca .5 >>'
    if kind in ('outer-alpha', 'combined-alpha'):
        prefix = b'/A gs '
    outer_gstate = b'<< /ca .5 /CA .75 >>'
    if equivalent_alpha and kind in ('ext-alpha', 'combined-alpha', 'background-alpha'):
        ext = b''
        prefix = b'/A gs '
        if kind == 'combined-alpha':
            outer_gstate = b'<< /ca .25 /CA .75 >>'
    form_resources = resources.replace(b'/Fm 10 0 R ', b'')
    form_object = stream(b'/Type /XObject /Subtype /Form /BBox [0 0 80 48] /Resources '+form_resources,
                         b'/Pattern cs /P scn 0 0 80 48 re f')
    if equivalent_alpha and kind in ('ext-alpha', 'combined-alpha', 'background-alpha'):
        prefix, select, paint = b'/A gs ', b'', b'/Fm Do'
        form_object = stream(b'/Type /XObject /Subtype /Form /BBox [0 0 96 64] '
                             b'/Group << /S /Transparency /CS /DeviceRGB /I true /K true >> /Resources '+form_resources,
                             b'/Pattern cs /P scn 4 4 88 56 re f')
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64] '+attrs+b'/Resources '+resources+b' /Contents 4 0 R >>',
        stream(b'', prefix+select+paint),
        b'<< /Type /Pattern /PatternType 2 /Shading 6 0 R /Matrix '+matrix+ext+b' >>',
        b'<< '+shader+b' >>',
        b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>',
        font,
        stream(b'/Type /XObject /Subtype /Image /ImageMask true /Width 4 /Height 4 /BitsPerComponent 1',bytes([0x30,0x30,0xc0,0xc0])),
        form_object,
        outer_gstate, *extra,
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for kind in CASES:
        target = root/('shading-pattern-'+kind+'.pdf')
        target.write_bytes(fixture(kind))
        if args.references:
            if kind in ('ext-alpha','outer-alpha','combined-alpha','background-alpha'):
                oracle = target
                if kind != 'outer-alpha':
                    oracle = target.with_name(target.stem+'-reference.pdf')
                    oracle.write_bytes(fixture(kind, equivalent_alpha=True))
                subprocess.run(['pdftoppm','-r','144','-singlefile','-png',str(oracle),str(target.with_suffix(''))],check=True)
            else:
                subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-dUseFastColor=true','-sDEVICE=png16m','-r144',
                                '-dGraphicsAlphaBits=1',f'-sOutputFile={target.with_suffix(".png")}',str(target)],check=True)
