"""Generate logical text-area metrics with the pinned Java OFDRW source."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
TEXTS = ["WiA", " A ", "", "\t\n\r", "中A"]
SOURCE = r'''
import java.nio.file.*;
import org.ofdrw.font.Font;
import org.ofdrw.layout.element.canvas.*;
public class CanvasTextAreaReference {
    public static void main(String[] args) throws Exception {
        String[] texts = {"WiA", " A ", "", "\t\n\r", "中A"};
        for (int rd : new int[]{0, 90, 180, 270}) {
            for (int cd : new int[]{0, 90, 180, 270}) {
                for (double spacing : new double[]{-0.5, 0, 0.25}) {
                    FontSetting font = new FontSetting(4, new Font("CanvasTest", Paths.get(args[0])))
                        .setReadDirection(rd).setCharDirection(cd).setLetterSpacing(spacing);
                    for (int i = 0; i < texts.length; i++) {
                        TextMetricsArea m = TextMeasureTool.measureArea(texts[i], font);
                        StringBuilder boxes = new StringBuilder("[");
                        for (java.awt.geom.Rectangle2D box : m.charAreas) {
                            if (boxes.length() > 1) boxes.append(',');
                            boxes.append("[0,0,").append(box.getWidth()).append(',')
                                .append(box.getHeight()).append(']');
                        }
                        boxes.append(']');
                        System.out.println("case:" + rd + "\t" + cd + "\t" + spacing + "\t" + i
                            + "\t" + m.width + "\t" + m.height + "\t" + m.letterSpacing + "\t" + boxes);
                    }
                }
            }
        }
    }
}
'''


def main():
    revision = subprocess.check_output(
        ['git', '-C', str(JAVA), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != PIN:
        raise SystemExit('Check out the pinned Java reference first')
    if not (JAVA / 'ofdrw-layout/target/classes').is_dir():
        raise SystemExit('Build the sibling Java layout module first')
    classpath = os.pathsep.join(map(str, [
        *sorted(JAVA.glob('*/target/classes')),
        *sorted((Path.home() / '.m2/repository').rglob('*.jar')),
    ]))
    font_root = ROOT / 'crates/ofdrw/tests/fixtures'
    fonts = ['dejavu-sans.ttf', 'dejavu-serif.ttf', 'kerning-simhei.ttf', 'test-name-cff.otf']
    with tempfile.TemporaryDirectory(prefix='ofdrw-canvas-area-') as temp:
        root = Path(temp)
        (root / 'CanvasTextAreaReference.java').write_text(SOURCE)
        compile_args = root / 'compile.args'
        compile_args.write_text('\n'.join(json.dumps(arg) for arg in [
            '-proc:none', '-cp', classpath, str(root / 'CanvasTextAreaReference.java'),
        ]) + '\n')
        subprocess.run(['javac', f'@{compile_args}'], check=True, capture_output=True, text=True)
        cases = []
        for name in fonts:
            args = ['-Djava.awt.headless=true', '-cp', os.pathsep.join([str(root), classpath]),
                    'CanvasTextAreaReference', str(font_root / name)]
            argument_file = root / 'run.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in args) + '\n')
            result = subprocess.run(['java', f'@{argument_file}'], check=True,
                                    capture_output=True, text=True)
            for line in result.stdout.splitlines():
                if line.startswith('case:'):
                    rd, cd, spacing, text, width, height, effective, boxes = line[5:].split('\t')
                    cases.append({
                        'font': name, 'read_direction': int(rd), 'char_direction': int(cd),
                        'letter_spacing': float(spacing), 'text': TEXTS[int(text)],
                        'width': float(width), 'height': float(height),
                        'effective_letter_spacing': float(effective), 'char_areas': json.loads(boxes),
                    })
    assert len(cases) == 960
    destination = ROOT / 'crates/ofdrw/tests/fixtures/canvas-text-java/areas.json'
    java_version = subprocess.run(['java', '-version'], capture_output=True, text=True,
                                  check=True).stderr.splitlines()[0]
    metadata = {'java_revision': PIN, 'java_version': java_version, 'font_size': 4}
    # One case per line keeps all 960 independent references reviewable.
    lines = ['{'] + [f'  {json.dumps(key)}: {json.dumps(value)},'
                     for key, value in metadata.items()] + ['  "cases": [']
    lines += ['    ' + json.dumps(case, ensure_ascii=False)
              + (',' if index + 1 < len(cases) else '')
              for index, case in enumerate(cases)]
    destination.write_text('\n'.join(lines + ['  ]', '}']) + '\n')


if __name__ == '__main__':
    main()
