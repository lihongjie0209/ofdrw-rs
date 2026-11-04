"""Author periodic and elliptic radial fields as native PDF shadings."""
import argparse
import json
import math
from pathlib import Path
import re
import subprocess

from generate_general_radial_fixtures import fixture
from generate_nested_stitching_fixture import pdf_bytes, stream


def periodic(reflect):
    objects = re.findall(rb'\d+ 0 obj\n(.*?)\nendobj', fixture('equal'), re.S)
    assert len(objects) == 14
    # Enlarge the swept center domain from s=[0,1] to [-2,3]. Each of five
    # subfunctions paints one original cycle; the radii stay constant.
    objects[4] = objects[4].replace(b'[24 32 14 72 32 14]', b'[-72 32 14 168 32 14]')
    function = b'<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>'
    encode = b' '.join(b'1 0' if reflect and i % 2 else b'0 1' for i in range(5))
    objects[5] = (b'<< /FunctionType 3 /Domain [0 1] /Functions [' + b' '.join([function] * 5)
                  + b'] /Bounds [.2 .4 .6 .8] /Encode [' + encode + b'] >>')
    return pdf_bytes(objects)


def ellipse():
    objects = re.findall(rb'\d+ 0 obj\n(.*?)\nendobj', fixture('equal'), re.S)
    b = .2 * math.sin(math.pi / 6) * math.cos(math.pi / 6)
    matrix = [0.95, -b, -b, 0.85, 72 * .05 + b * 32, 32 * .15 + b * 72]
    transform = ' '.join(format(value, '.16g') for value in matrix).encode()
    objects[3] = stream(b'', b'q ' + transform + b' cm /S sh Q')
    return pdf_bytes(objects)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--references', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1] / 'crates/ofdrw/tests/fixtures'
    for name, data in [('equal-repeat', periodic(False)), ('equal-reflect', periodic(True)), ('ellipse', ellipse())]:
        target = root / f'general-radial-{name}.pdf'
        target.write_bytes(data)
        if args.references:
            subprocess.run(['gs', '-q', '-dSAFER', '-dBATCH', '-dNOPAUSE', '-sDEVICE=png16m',
                            '-r144', f'-sOutputFile={target.with_suffix(".png")}', str(target)], check=True)
    if args.references:
        (root / 'radial-paint-reference.json').write_text(json.dumps({
            'oracle': 'Original native PDF rendered by Ghostscript',
            'ghostscript_version': subprocess.check_output(['gs', '--version'], text=True).strip(),
            'dpi': 144, 'cases': ['equal-repeat', 'equal-reflect', 'ellipse'],
            'construction': 'Five stitching-function cycles over center domain [-2,3]; rotated eccentric ellipse via an independent PDF affine matrix'
        }, indent=2) + '\n')


if __name__ == '__main__':
    main()
