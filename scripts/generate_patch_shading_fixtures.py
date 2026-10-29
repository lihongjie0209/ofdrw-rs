"""Author PDF Coons/tensor patches and optional independent Ghostscript references."""

import argparse
from pathlib import Path
import subprocess
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream


def line(a, b):
    return [tuple(a[axis] * (1-t) + b[axis] * t for axis in range(2))
            for t in [0, 1/3, 2/3]]


def quad(a, b, c, d):
    boundary = line(a, b) + line(b, c) + line(c, d) + line(d, a)
    interior = []
    for u, v in [(1/3, 1/3), (1/3, 2/3), (2/3, 2/3), (2/3, 1/3)]:
        interior.append(tuple((1-u)*(1-v)*a[axis] + (1-u)*v*b[axis]
                              + u*v*c[axis] + u*(1-v)*d[axis] for axis in range(2)))
    return boundary + interior


def patch_pdf(kind=6, curved=False, reuse=0, function=False, transformed=False,
              folded=False, coordinate_bits=12, component_bits=8, flag_bits=2,
              space=b'/DeviceRGB', corner_colors=None):
    points = quad((12, 12), (12, 84), (84, 84), (84, 12))
    if curved:
        points[:12] = [(12,12), (0,32), (28,64), (12,84), (32,96), (64,68),
                       (84,84), (96,64), (68,32), (84,12), (64,0), (32,28)]
        points[12:] = [(20,20), (40,84), (80,64), (56,12)]
    if folded:
        points = quad((24,12), (24,84), (72,84), (72,12))
        points[12:] = [(96,24), (96,72), (0,72), (0,24)]
    colors = corner_colors or [(1,0,0), (0,1,0), (0,0,1), (1,1,0)]
    if function:
        colors = [(0,), (1,), (.2,), (.8,)]
    patches = [(0, points, colors)]
    if reuse:
        points = quad((30,30), (30,60), (60,60), (60,30))
        patches = [(0, points, colors)]
        edge = [points[(reuse*3+i) % 12] for i in range(4)]
        delta = {1:(0,28), 2:(28,0), 3:(0,-22)}[reuse]
        a, b = edge[0], edge[3]
        c = tuple(b[i]+delta[i] for i in range(2))
        d = tuple(a[i]+delta[i] for i in range(2))
        second = quad(a,b,c,d)
        second[:4] = edge
        patches.append((reuse, second, [colors[reuse], colors[(reuse+1)%4], colors[0], colors[1]]))
    fields = []
    for flag, points, colors in patches:
        fields.append((flag, flag_bits))
        for point in points[(4 if flag else 0):(16 if kind == 7 else 12)]:
            fields.extend((round(value/96*((1 << coordinate_bits)-1)), coordinate_bits) for value in point)
        for color in colors[(2 if flag else 0):]:
            fields.extend((round(value*((1 << component_bits)-1)), component_bits) for value in color)
    bits = ''.join(f'{value:0{width}b}' for value, width in fields)
    bits += '0' * (-len(bits) % 8)
    data = int(bits,2).to_bytes(len(bits)//8,'big')
    dictionary = (f'/ShadingType {kind} /BitsPerCoordinate {coordinate_bits} '
                  f'/BitsPerComponent {component_bits} /BitsPerFlag {flag_bits} '
                  '/Decode [0 96 0 96').encode() + b' 0 1' * len(colors[0]) + b'] /Filter /FlateDecode /ColorSpace ' + space
    calculator = b'null'
    if function:
        dictionary += b' /Function 6 0 R'
        calculator = b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 2 >>'
    content = b'/Sh1 sh'
    if transformed:
        content = b'q 12 12 70 68 re W n 1 .15 -.1 .85 4 2 cm /GS gs /Sh1 sh Q'
        dictionary += b' /BBox [10 10 86 86] /Background [1 0 1]'
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Contents 4 0 R '
        b'/Resources << /Shading << /Sh1 5 0 R >> /ExtGState << /GS << /ca .5 /BM /Multiply >> >> >> >>',
        stream(b'',content), stream(dictionary,zlib.compress(data)), calculator,
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures'
    cases = [
        ('patch-coons-calgray', {'curved': True,
            'space': b'[/CalGray << /WhitePoint [.9505 1 1.089] /Gamma 1 >>]',
            'corner_colors': [(0,), (1,), (.2,), (.8,)]}),
        ('patch-tensor-cmyk', {'kind': 7, 'curved': True, 'space': b'/DeviceCMYK',
            'corner_colors': [(0,1,1,0), (1,0,1,.2), (1,1,0,.5), (0,0,0,1)]}),
        ('patch-coons-flat', {}),
        ('patch-coons-curved', {'curved': True}),
        ('patch-tensor-curved', {'kind': 7, 'curved': True}),
        ('patch-tensor-folded', {'kind': 7, 'folded': True}),
        ('patch-coons-reuse-1', {'reuse': 1}),
        ('patch-coons-reuse-2', {'reuse': 2}),
        ('patch-coons-reuse-3', {'reuse': 3}),
        ('patch-tensor-reuse', {'kind': 7, 'reuse': 2}),
        ('patch-coons-function', {'curved': True, 'function': True}),
        ('patch-tensor-clipped', {'kind': 7, 'curved': True, 'transformed': True}),
        ('patch-coons-wide', {'coordinate_bits': 32, 'component_bits': 16, 'flag_bits': 8}),
        ('patch-coons-packed', {'coordinate_bits': 12, 'component_bits': 4, 'flag_bits': 4}),
    ]
    for name, options in cases:
        target = root / f'{name}.pdf'
        target.write_bytes(patch_pdf(**options))
        if args.references:
            fast_color = ['-dUseFastColor'] if name == 'patch-tensor-cmyk' else []
            subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE',*fast_color,'-sDEVICE=png16m','-r72',
                            '-dGraphicsAlphaBits=1',f'-sOutputFile={root / (name + ".png")}',str(target)],check=True)
