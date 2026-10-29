"""Author nonconformal direct shadings and original-PDF Ghostscript references."""
from pathlib import Path
import argparse
import subprocess
from generate_nested_stitching_fixture import pdf_bytes, stream

CASES = [f'{kind}-{case}' for kind in ['axial','radial'] for case in
         ['shear','anisotropic','clip','alpha','background','form','rotate','user-unit','reflect-shear','multiply','small-shear']]
CASES += ['hidden-axial','hidden-radial']


def fixture(name):
    radial = 'radial' in name
    matrix = b'1 .3 .4 .8 -10 -5'
    if 'small-shear' in name:
        matrix = b'1 0 .0000001 1 0 0'
    elif 'anisotropic' in name:
        matrix = b'1.4 0 0 .7 -20 10'
    elif 'reflect-shear' in name:
        matrix = b'-1 .3 .4 .8 80 -5'
    shader = (b'/ShadingType 3 /ColorSpace /DeviceRGB /Coords [48 32 0 48 32 28]'
              if radial else b'/ShadingType 2 /ColorSpace /DeviceRGB /Coords [12 12 84 52]')
    shader += b' /Function 6 0 R /Extend [true true]'
    prefix = b''
    attrs = b''
    if 'clip' in name:
        prefix = b'8 8 80 48 re W n '
        shader += b' /BBox [20 12 80 52]'
    if 'background' in name:
        shader = shader.replace(b'[true true]',b'[false false]') + b' /Background [0 1 0]'
    if 'alpha' in name:
        prefix += b'/G gs '
    if 'multiply' in name:
        prefix = b'1 1 0 rg 0 0 96 64 re f /G gs '
    if 'rotate' in name:
        attrs = b'/Rotate 90 '
    elif 'user-unit' in name:
        attrs = b'/UserUnit 2 '
    resources = (b'<< /Shading << /S 5 0 R >> /ExtGState << /G 7 0 R >> '
                 b'/XObject << /Fm 8 0 R >> /Properties << /H 9 0 R >> >>')
    content = prefix+b'q '+matrix+b' cm /S sh Q'
    if 'form' in name:
        content = prefix+b'/Fm Do'
    if name.startswith('hidden'):
        content = b'/OC /H BDC '+content+b' EMC'
    catalog = b'<< /Type /Catalog /Pages 2 0 R'
    if name.startswith('hidden'):
        catalog += b' /OCProperties << /OCGs [9 0 R] /D << /BaseState /OFF /OFF [9 0 R] >> >>'
    catalog += b' >>'
    gs = b'<< /ca .5 /BM /Multiply >>' if 'multiply' in name else b'<< /ca .5 >>'
    return pdf_bytes([
        catalog, b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 64] '+attrs+b'/Resources '+resources+b' /Contents 4 0 R >>',
        stream(b'',content), b'<< '+shader+b' >>',
        b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>',
        gs,
        stream(b'/Type /XObject /Subtype /Form /BBox [0 0 96 64] /Matrix ['+matrix+b'] '
               b'/Resources << /Shading << /S 5 0 R >> /ExtGState << /G 7 0 R >> >>',b'/S sh'),
        b'<< /Type /OCG /Name (Hidden shading) >>',
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'crates/ofdrw/tests/fixtures'
    for name in CASES:
        target = root/f'affine-shading-{name}.pdf'
        target.write_bytes(fixture(name))
        if args.references:
            subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE','-sDEVICE=png16m','-r144',
                            '-dGraphicsAlphaBits=1',f'-sOutputFile={target.with_suffix(".png")}',str(target)],check=True)
