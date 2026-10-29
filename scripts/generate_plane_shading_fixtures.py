"""Author two-input function shadings with independent raster references."""

import argparse
from pathlib import Path
import subprocess
import zlib

from generate_nested_stitching_fixture import pdf_bytes, stream


def calculator(code, channels=3, domain=b'0 1 0 1'):
    return stream(b'/FunctionType 4 /Domain [' + domain + b'] /Range ['
                  + b'0 1 ' * channels + b']', code)


def sampled(bits=8, size=(2, 2), cubic=False, reverse=False):
    values = []
    for y in range(size[1]):
        for x in range(size[0]):
            if cubic:
                red = ((x*x+3*y) % 7)/6
                color = [red, ((x+2*y) % 5)/4, 1-red]
            else:
                color = [[1,0,0], [0,1,0], [0,0,1], [1,1,0]][(y*size[0]+x) % 4]
            values.extend(round(c*((1 << bits)-1)) for c in color)
    packed = ''.join(f'{value:0{bits}b}' for value in values)
    packed += '0' * (-len(packed) % 8)
    dictionary = (f'/FunctionType 0 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] '
                  f'/Size [{size[0]} {size[1]}] /BitsPerSample {bits} '
                  f'/Order {3 if cubic else 1} /Filter /FlateDecode').encode()
    if reverse:
        dictionary += b' /Encode [1 0 1 0] /Decode [1 0 1 0 1 0]'
    return stream(dictionary, zlib.compress(int(packed,2).to_bytes(len(packed)//8,'big')))


def fixture(name):
    dictionary = b'/ShadingType 1 /ColorSpace /DeviceRGB /Matrix [72 0 0 72 12 12] /Function 6 0 R'
    function = calculator(b'{ 1 index 1 exch sub }')
    extra = []
    content = b'/Sh1 sh'
    if name == 'plane-sampled-rgb':
        function = sampled()
    elif name == 'plane-sampled-packed':
        function = sampled(bits=12, size=(3,2))
    elif name == 'plane-sampled-cubic':
        function = sampled(bits=16, size=(5,4), cubic=True)
    elif name == 'plane-sampled-cubic-small':
        function = sampled(bits=12, size=(3,3), cubic=True)
    elif name == 'plane-sampled-reversed':
        function = sampled(reverse=True)
    elif name == 'plane-calculator-step':
        function = calculator(b'{ exch .5 lt { pop 1 0 0 } { pop 0 0 1 } ifelse }')
    elif name == 'plane-component-array':
        function = b'[7 0 R 8 0 R 9 0 R]'
        extra = [calculator(b'{ pop }',1), calculator(b'{ exch pop }',1),
                 calculator(b'{ pop 1 exch sub }',1)]
    elif name == 'plane-cmyk':
        dictionary = dictionary.replace(b'/DeviceRGB', b'/DeviceCMYK')
        function = calculator(b'{ .25 .1 }',4)
    elif name == 'plane-calgray':
        dictionary = dictionary.replace(b'/DeviceRGB', b'[/CalGray << /WhitePoint [.9505 1 1.089] /Gamma 1 >>]')
        function = calculator(b'{ add .5 mul }',1)
    elif name == 'plane-transformed':
        dictionary = dictionary.replace(b'[72 0 0 72 12 12]', b'[24 2 -1 12 48 48] /Domain [-1 1 -2 2]')
        dictionary += b' /BBox [12 12 84 84]'
        content = b'q 18 14 64 68 re W n 1 .1 -.15 .9 8 4 cm /GS gs /Sh1 sh Q'
        function = calculator(b'{ 2 add .25 mul exch 1 add .5 mul exch 1 index 1 exch sub }',
                              domain=b'-2 2 -3 3')
    elif name == 'plane-bbox-background':
        dictionary = dictionary.replace(b'[72 0 0 72 12 12]', b'[48 0 0 48 24 24]')
        dictionary += b' /BBox [8 8 88 88] /Background [0 1 0]'
    return pdf_bytes([
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 96 96] /Contents 4 0 R '
        b'/Resources << /Shading << /Sh1 5 0 R >> /ExtGState << /GS << /ca .5 /BM /Multiply >> >> >> >>',
        stream(b'',content), b'<< '+dictionary+b' >>', function, *extra,
    ])


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--references',action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures'
    for name in ['plane-calculator-rgb','plane-calculator-step','plane-component-array','plane-cmyk',
                 'plane-sampled-rgb','plane-sampled-packed','plane-sampled-cubic',
                 'plane-sampled-cubic-small','plane-sampled-reversed','plane-calgray','plane-transformed','plane-bbox-background']:
        target = root / (name+'.pdf')
        target.write_bytes(fixture(name))
        if args.references:
            fast_color = ['-dUseFastColor'] if name == 'plane-cmyk' else []
            subprocess.run(['gs','-q','-dSAFER','-dBATCH','-dNOPAUSE',*fast_color,'-sDEVICE=png16m','-r72',
                            '-dGraphicsAlphaBits=1',f'-sOutputFile={root/(name+".png")}',str(target)],check=True)
