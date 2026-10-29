"""Generate authored PDF Gouraud meshes; optional independent Ghostscript rasters."""

import argparse
from pathlib import Path
import subprocess
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream


def record(flag, x, y, color, coordinate_bits, component_bits, flag_bits):
    fields = ([(flag, flag_bits)] if flag_bits else [])
    fields += [(round(x * ((1 << coordinate_bits) - 1)), coordinate_bits),
               (round(y * ((1 << coordinate_bits) - 1)), coordinate_bits)]
    fields += [(round(c * ((1 << component_bits) - 1)), component_bits) for c in color]
    bits = ''.join(f'{value:0{width}b}' for value, width in fields)
    bits += '0' * (-len(bits) % 8)
    return int(bits, 2).to_bytes(len(bits) // 8, 'big')


def mesh(kind=4, coordinate_bits=8, component_bits=8, flag_bits=2,
         space=b'/DeviceRGB', colors=None, function=None, transformed=False):
    colors = colors or [(1, 0, 0), (0, 1, 0), (0, 0, 1), (1, 1, 0), (0, 1, 1), (1, 0, 1)]
    # Type 4 includes two reused edges followed by a separate triangle. The
    # flags on the second and third vertices of a new triangle are ignored.
    vertices = [(0, 0, 0), (3, .5, 0), (3, 0, 1), (1, .5, 1),
                (2, 1, 1), (0, .65, .1), (3, 1, .1), (3, 1, .8)]
    if kind == 5:
        vertices = [(0, 0, 0), (0, .5, 0), (0, 1, 0),
                    (0, 0, 1), (0, .5, 1), (0, 1, 1)]
    data = b''.join(record(flag, x, y, colors[index % len(colors)], coordinate_bits,
                           component_bits, flag_bits if kind == 4 else 0)
                    for index, (flag, x, y) in enumerate(vertices))
    dictionary = (f'/ShadingType {kind} /BitsPerCoordinate {coordinate_bits} '
                  f'/BitsPerComponent {component_bits} ').encode() + b'/ColorSpace ' + space
    dictionary += (f' /BitsPerFlag {flag_bits}'.encode() if kind == 4 else b' /VerticesPerRow 3')
    dictionary += b' /Decode [8 88 8 88' + b' 0 1' * len(colors[0]) + b'] /Filter /FlateDecode'
    if function:
        dictionary += b' /Function 6 0 R'
    content = b'/Sh1 sh'
    if transformed:
        content = b'q 12 12 70 68 re W n 1 .15 -.1 .85 4 2 cm /GS gs /Sh1 sh Q'
        dictionary += b' /BBox [10 10 86 86] /Background [1 0 1]'
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Contents 4 0 R '
        b'/Resources << /Shading << /Sh1 5 0 R >> /ExtGState << /GS << /ca .5 /BM /Multiply >> >> >> >>',
        stream(b'', content), stream(dictionary, zlib.compress(data)), function or b'null',
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures'
    cases = [
        ('mesh-free-rgb', {}),
        ('mesh-lattice-rgb', {'kind': 5}),
        ('mesh-packed-gray', {'coordinate_bits': 12, 'component_bits': 4, 'flag_bits': 4,
                              'space': b'/DeviceGray', 'colors': [(0,), (1,), (.2,), (.8,)]}),
        ('mesh-wide-rgb', {'coordinate_bits': 32, 'component_bits': 16, 'flag_bits': 8}),
        ('mesh-lattice-clipped', {'kind': 5, 'transformed': True}),
        ('mesh-cmyk', {'space': b'/DeviceCMYK', 'colors': [(0, 1, 1, 0), (1, 0, 1, .2),
                                                      (1, 1, 0, .5), (0, 0, 0, 1)]}),
        ('mesh-calgray', {'space': b'[/CalGray << /WhitePoint [.9505 1 1.089] /Gamma 2.2 >>]',
                          'colors': [(0,), (1,), (.25,), (.75,)]}),
        ('mesh-function-array', {'colors': [(0,), (1,), (.2,), (.8,)],
             'function': b'[<< /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 2 >> '
                         b'<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [0] /N 1 >> '
                         b'<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 2 >>]'}),
        ('mesh-sampled-function', {'colors': [(0,), (1,), (.2,), (.8,)],
             'function': stream(b'/FunctionType 0 /Domain [0 1] /Range [0 1 0 1 0 1] '
                                b'/Size [3] /BitsPerSample 8', bytes([255,0,0,0,255,0,0,0,255]))}),
        ('mesh-stitching-function', {'colors': [(0,), (1,), (.2,), (.8,)],
             'function': b'<< /FunctionType 3 /Domain [0 1] /Bounds [.5] /Encode [0 1 0 1] /Functions ['
                         b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 1 0] /N 1 >> '
                         b'<< /FunctionType 2 /Domain [0 1] /C0 [0 1 0] /C1 [0 0 1] /N 1 >>] >>'}),
        ('mesh-calculator-function', {'colors': [(0,), (1,), (.2,), (.8,)],
             'function': stream(b'/FunctionType 4 /Domain [0 1] /Range [0 1 0 1 0 1]',
                                b'{ dup mul dup 1 exch sub exch 0 exch }')}),
        ('mesh-function', {'colors': [(0,), (1,), (.2,), (.8,)],
                           'function': b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 2 >>'}),
    ]
    for name, options in cases:
        target = root / f'{name}.pdf'
        target.write_bytes(mesh(**options))
        if args.references:
            # DeviceCMYK uses the PDF's simple subtractive conversion, without
            # Ghostscript's installation-specific default CMYK ICC profile.
            fast_color = ['-dUseFastColor'] if name == 'mesh-cmyk' else []
            subprocess.run(['gs', '-q', '-dSAFER', '-dBATCH', '-dNOPAUSE', *fast_color, '-sDEVICE=png16m',
                            '-r72', '-dGraphicsAlphaBits=1', f'-sOutputFile={root / (name + ".png")}',
                            str(target)], check=True)
