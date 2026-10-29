"""Author general two-circle PDF fields and original-source Ghostscript rasters."""
from pathlib import Path
import argparse
import subprocess
from generate_nested_stitching_fixture import pdf_bytes,stream

CASES = ['grow','shrink','overlap','equal','grow-start','grow-end','grow-both',
         'shrink-start','shrink-end','shrink-both','equal-both','point-start','point-end',
         'both-zero','tangent','near-tangent','affine','reflected','clipped','alpha','multiply',
         'pattern-background','pattern-affine','pattern-scaled','pattern-zero-background',
         'nested-pattern','sampled','calculator-step','component-array','cmyk','calgray','hidden','form']


def fixture(name):
    coords=b'[24 32 8 72 32 20]'
    extend=b'[false false]'
    if name.startswith('shrink'):
        coords=b'[24 32 20 72 32 8]'
    elif name.startswith('equal'):
        coords=b'[24 32 14 72 32 14]'
    elif name=='overlap':
        coords=b'[32 32 18 60 32 24]'
    elif name=='point-start':
        coords=b'[24 32 0 72 32 20]'
    elif name=='point-end':
        coords=b'[24 32 20 72 32 0]'
    elif name in ('both-zero','pattern-zero-background'):
        coords=b'[24 32 0 72 32 0]'
    elif name=='tangent':
        coords=b'[24 32 4 40 32 20]'
    elif name=='near-tangent':
        coords=b'[24 32 4 40.0001 32 20]'
    if name.endswith('-start') and name not in ('point-start',):
        extend=b'[true false]'
    elif name.endswith('-end') and name not in ('point-end',):
        extend=b'[false true]'
    elif name.endswith('-both'):
        extend=b'[true true]'
    content=b'/S sh'
    matrix=b'[1 0 0 1 0 0]'
    prefix=b''
    background=b''
    bbox=b''
    space=b'/DeviceRGB'
    function=b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>'
    if name in ('affine','pattern-affine'):
        matrix=b'[1 .2 .4 .8 -8 0]'
    elif name=='reflected':
        matrix=b'[-1 .2 .3 .9 84 0]'
    elif name=='pattern-scaled':
        matrix=b'[3 0 0 3 -96 -64]'
    if name in ('affine','reflected'):
        content=b'q '+matrix[1:-1]+b' cm /S sh Q'
    elif name=='clipped':
        prefix=b'8 8 80 48 re W n '
        bbox=b' /BBox [20 14 78 52]'
    elif name=='alpha':
        prefix=b'/G gs '
    elif name=='multiply':
        prefix=b'1 1 0 rg 0 0 96 64 re f /G gs '
    elif name=='sampled':
        function=stream(b'/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1] /Size [3] /BitsPerSample 8',bytes([255,0,0,0,255,0,0,0,255]))
    elif name=='calculator-step':
        function=stream(b'/FunctionType 4 /Domain [0 1] /Range [0 1 0 1 0 1]',b'{ .5 lt { 1 0 0 } { 0 0 1 } ifelse }')
    elif name=='component-array':
        function=b'[10 0 R 11 0 R 12 0 R]'
    elif name=='cmyk':
        space=b'/DeviceCMYK'
        function=b'<< /FunctionType 2 /Domain [0 1] /C0 [0 1 1 0] /C1 [1 1 0 0] /N 1 >>'
    elif name=='calgray':
        space=b'[/CalGray << /WhitePoint [.9505 1 1.089] /Gamma 2.2 >>]'
        function=b'<< /FunctionType 2 /Domain [0 1] /C0 [.1] /C1 [.9] /N 1 >>'
    if name.startswith('pattern'):
        content=b'/Pattern cs /P scn 4 4 88 56 re f'
        if 'background' in name:
            background=b' /Background [.2 .8 .3]'
    elif name=='nested-pattern':
        content=b'/Pattern cs /T scn 0 0 96 64 re f'
        matrix=b'[.5 0 0 .5 0 0]'
    elif name=='hidden':
        content=b'/OC /H BDC /S sh EMC'
    elif name=='form':
        content=b'/F Do'
    catalog=b'<< /Type /Catalog /Pages 2 0 R'
    if name=='hidden':
        catalog+=b' /OCProperties << /OCGs [13 0 R] /D << /BaseState /OFF /OFF [13 0 R] >> >>'
    catalog+=b' >>'
    return pdf_bytes([
        catalog,b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64] /Resources << /Shading << /S 5 0 R >> '
        b'/Pattern << /P 8 0 R /T 9 0 R >> /ExtGState << /G 7 0 R >> /Properties << /H 13 0 R >> /XObject << /F 14 0 R >> >> /Contents 4 0 R >>',
        stream(b'',prefix+content),
        b'<< /ShadingType 3 /ColorSpace '+space+b' /Coords '+coords+b' /Extend '+extend+b' /Function 6 0 R'+bbox+background+b' >>',
        function,b'<< /ca .5'+(b' /BM /Multiply' if name=='multiply' else b'')+b' >>',
        b'<< /Type /Pattern /PatternType 2 /Shading 5 0 R /Matrix '+matrix+b' >>',
        stream(b'/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 2 /BBox [0 0 48 32] /XStep 48 /YStep 32 /Matrix [2 0 0 2 0 0] '
               b'/Resources << /Pattern << /S 8 0 R >> >>', b'/Pattern cs /S scn 0 0 48 32 re f'),
        b'<< /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 2 >>',
        b'<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [0] /N 1 >>',
        b'<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 2 >>',
        b'<< /Type /OCG /Name (Hidden radial field) >>',
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 96 64] /Matrix [1 .2 .4 .8 -8 0] /Resources << /Shading << /S 5 0 R >> >>',b'/S sh')
    ])


if __name__=='__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args=parser.parse_args()
    root=Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for name in CASES:
        target=root/f'general-radial-{name}.pdf'
        target.write_bytes(fixture(name))
        if args.references:
            subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-dUseFastColor=true','-sDEVICE=png16m','-r144',
                            '-dGraphicsAlphaBits=1',f'-sOutputFile={target.with_suffix(".png")}',str(target)],check=True)
