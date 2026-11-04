"""Generate independent Java2D clip geometry and retain raw Java OFDRW output."""
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = '84e9eb260ba6fd351ecccf08a01a40305d4af635'
IDENTITY = [1, 0, 0, 1, 0, 0]


def matrix(value):
    return {'type': 'set_transform', 'matrix': value}


def rect(value):
    return {'type': 'rect', 'rect': value}


def fill(value=(0, 0, 28, 22), color=(40, 90, 200)):
    return {'type': 'fill_rect', 'rect': list(value), 'color': list(color)}


CASES = [
    {'name': 'sheared-rect', 'commands': [
        matrix([0.9, 0.4, 0.3, 1.1, 5, 3]), rect([0, 0, 10, 8]), {'type': 'clip'},
        {'type': 'begin_path'}, matrix([1, 0.2, -0.15, 1, 0, 0]), fill()]},
    {'name': 'reflected-curves', 'commands': [
        matrix([-1, 0.3, 0.2, 1, 23, 2]), {'type': 'move_to', 'point': [0, 2]},
        {'type': 'bezier_curve_to', 'control1': [3, -1], 'control2': [10, -1], 'end': [12, 4]},
        {'type': 'quadratic_curve_to', 'control': [10, 11], 'end': [2, 9]},
        {'type': 'close_path'}, {'type': 'clip'}, {'type': 'begin_path'}, matrix(IDENTITY), fill()]},
    {'name': 'replacement', 'commands': [
        rect([1, 1, 5, 5]), {'type': 'clip'}, {'type': 'begin_path'},
        matrix([1, 0.25, -0.2, 1, 13, 4]), rect([0, 0, 9, 8]), {'type': 'clip'},
        {'type': 'begin_path'}, matrix(IDENTITY), fill()]},
    {'name': 'save-restore', 'commands': [
        matrix([1, 0.2, 0.1, 1, 4, 2]), rect([0, 0, 17, 14]), {'type': 'clip'}, {'type': 'save'},
        {'type': 'begin_path'}, matrix([0.8, -0.3, 0.4, 0.9, 6, 8]), rect([0, 0, 9, 9]),
        {'type': 'clip'}, {'type': 'begin_path'}, matrix(IDENTITY), fill(color=(40, 180, 90)),
        {'type': 'restore'}, matrix([1, 0.15, -0.1, 1, 2, 0]), fill((0, 0, 12, 22), (200, 50, 80))]},
    {'name': 'sheared-circle', 'commands': [
        matrix([1.1, 0.4, 0.6, 0.9, 2, 2]),
        {'type': 'circle_arc', 'center': [8, 7], 'radius': 5}, {'type': 'close_path'},
        {'type': 'clip'}, {'type': 'begin_path'}, matrix(IDENTITY), fill()]},
]


def main():
    revision = subprocess.check_output(['git', '-C', str(JAVA), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != PIN:
        raise RuntimeError('Java source is not at the reference pin')
    destination = ROOT/'crates/ofdrw/tests/fixtures/canvas-affine-clips-java'
    destination.mkdir(exist_ok=True)
    # Use Maven's tested dependency versions rather than mixing every cached jar.
    report = JAVA/'ofdrw-converter/target/surefire-reports/TEST-OFD2IMGTest.xml'
    if not report.exists():
        raise RuntimeError('Build and run the pinned Java converter OFD2IMGTest first')
    properties = ET.parse(report).getroot().find('properties')
    classpath = next(node.get('value') for node in properties if node.get('name') == 'java.class.path')
    entries = [entry for entry in classpath.split(os.pathsep) if Path(entry).exists() and '/target/test-classes' not in entry]
    classpath = os.pathsep.join(entries)
    scenes = ET.Element('scenes')
    for case in CASES:
        scene = ET.SubElement(scenes, 'scene', {'name': case['name']})
        for command in case['commands']:
            ET.SubElement(scene, command['type'], {key: ' '.join(map(str, value)) if isinstance(value, list) else str(value)
                for key, value in command.items() if key != 'type'})
    with tempfile.TemporaryDirectory(prefix='ofdrw-affine-clips-') as temporary:
        root = Path(temporary)
        requests = root/'requests.xml'
        ET.ElementTree(scenes).write(requests, encoding='utf-8')
        arguments = [('-proc:none', '-cp', classpath, '-d', str(root),
            str(ROOT/'scripts/java/CanvasAffineClipReference.java')),
            ('-Djava.awt.headless=true', '-cp', str(root)+os.pathsep+classpath,
             'CanvasAffineClipReference', str(requests), str(root))]
        for executable, args in zip(['javac', 'java'], arguments):
            argument_file = root/f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in args), encoding='utf-8')
            subprocess.run([executable, f'@{argument_file}'], check=True)
        for case in CASES:
            with zipfile.ZipFile(root/(case['name']+'.ofd')) as source:
                with zipfile.ZipFile(root/(case['name']+'-no-clips.ofd'), 'w', zipfile.ZIP_DEFLATED) as stripped:
                    removed = 0
                    for entry in source.infolist():
                        content = source.read(entry.filename)
                        if entry.filename.endswith('Content.xml'):
                            # Preserve all other bytes, including the ofd namespace prefix:
                            # the pinned Java reader fails to read ET's renamed prefixes.
                            content, count = re.subn(rb'<ofd:Clips(?:\s[^>]*)?>.*?</ofd:Clips>', b'', content, flags=re.DOTALL)
                            removed += count
                        stripped.writestr(entry, content)
                    assert removed > 0
        argument_file = root/'render-no-clips.args'
        argument_file.write_text('\n'.join(json.dumps(arg) for arg in (
            '-Djava.awt.headless=true', '-cp', str(root)+os.pathsep+classpath,
            'CanvasAffineClipReference', '--no-clips', str(root))), encoding='utf-8')
        subprocess.run(['java', f'@{argument_file}'], check=True)
        for case in CASES:
            for suffix in ['.ofd', '-geometry-4.png', '-geometry-12.png', '-imagemaker-4.png', '-imagemaker-no-clips-4.png']:
                name = case['name']+suffix
                (destination/name).write_bytes((root/name).read_bytes())
    metadata = {'java_runtime': subprocess.check_output(['java', '-version'], stderr=subprocess.STDOUT, text=True).splitlines(),
        'java_commit': PIN, 'page_size': [40, 30], 'boundary': [4, 3, 28, 22], 'pixels_per_mm': [4, 12],
        'geometry_oracle': 'Independent Java2D Area/BasicStroke/AffineTransform operations; no Rust execution.',
        'raw_ofdrw': 'Unmodified DrawContext outputs retained. ImageMaker ignores object Clips and is a negative control, not the geometry oracle.',
        'arc_difference': 'Pinned Java DrawContext approximates transformed arc radii; the Java2D ellipse reference retains full affine geometry.',
        'renderer_dependencies': [Path(entry).name for entry in entries if entry.endswith('.jar')],
        'draw_param_workaround': 'Explicitly reset fill, stroke and width before each paint; pinned Java restore loses drawParam.',
        'cases': CASES}
    (destination/'reference.json').write_text(json.dumps(metadata, indent=2)+'\n', encoding='utf-8')
    print('Generated affine clip scenes:', len(CASES))


if __name__ == '__main__':
    main()
