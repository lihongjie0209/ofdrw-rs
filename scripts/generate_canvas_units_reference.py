"""Execute pinned Java DrawContext pixel/mm conversions, retaining float bits."""
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = r'''
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.element.canvas.*;
public class CanvasUnitsReference {
    public static void main(String[] args) throws Exception {
        try (OFDDoc doc = new OFDDoc(Paths.get(args[1]))) {
            VirtualPage page = new VirtualPage(40d,30d);
            Canvas canvas = new Canvas(30d,20d);
            canvas.setPosition(Position.Absolute).setX(0d).setY(0d).setBorder(0d);
            canvas.setDrawer(c -> {
                for (String row : Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8)) {
                    String[] fields = row.split("\t", -1);
                    c.PPM = Double.longBitsToDouble(Long.parseUnsignedLong(fields[1],16));
                    if (fields[0].equals("mm")) {
                        double result = c.mm(Integer.parseInt(fields[2]));
                        System.out.println("UNIT\t"+Long.toUnsignedString(Double.doubleToRawLongBits(result),16));
                    } else {
                        double input = Double.longBitsToDouble(Long.parseUnsignedLong(fields[2],16));
                        System.out.println("UNIT\t"+c.pixel(input));
                    }
                }
            });
            page.add(canvas); doc.addVPage(page);
        }
    }
}
'''


def bits(value):
    return struct.pack('>d', value).hex()


def main():
    revision = subprocess.check_output(['git', '-C', str(JAVA), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != PIN:
        raise RuntimeError('Java source is not at the reference pin')
    cases = []
    for ppm in [1., 3.78, 4., 96. / 25.4, 300. / 25.4, 5e-324, 1.7976931348623157e308]:
        for pixel in [-(2**31), -(2**31)+1, -10000, -378, -12, -1, 0, 1, 12, 378, 10000, 2**31-2, 2**31-1]:
            cases.append({'op': 'mm', 'ppm_bits': bits(ppm), 'input': pixel})
        values = [-math.inf, math.inf, math.nan, -0., 0., -1e300, 1e300, -3.17, 3.17, -0.49, 0.49, -1., 1.]
        for boundary in [-2**31-1, -2**31, -1, 0, 1, 2**31-1, 2**31]:
            center = boundary / ppm
            values += [math.nextafter(center, -math.inf), center, math.nextafter(center, math.inf)]
        for mm in values:
            cases.append({'op': 'pixel', 'ppm_bits': bits(ppm), 'input_bits': bits(mm)})
    classpath = os.pathsep.join(map(str, [*sorted(JAVA.glob('*/target/classes')),
        *sorted((Path.home()/'.m2/repository').rglob('*.jar'))]))
    with tempfile.TemporaryDirectory(prefix='ofdrw-canvas-units-') as temporary:
        root = Path(temporary)
        (root/'CanvasUnitsReference.java').write_text(SOURCE, encoding='utf-8')
        requests = root/'requests.tsv'
        requests.write_text('\n'.join(f"{c['op']}\t{c['ppm_bits']}\t{c.get('input', c.get('input_bits'))}" for c in cases), encoding='utf-8')
        arguments = [('-proc:none', '-cp', classpath, str(root/'CanvasUnitsReference.java')),
            ('-Djava.awt.headless=true', '-cp', str(root)+os.pathsep+classpath,
             'CanvasUnitsReference', str(requests), str(root/'reference.ofd'))]
        output = ''
        for executable, args in zip(['javac', 'java'], arguments):
            argument_file = root/f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in args), encoding='utf-8')
            output = subprocess.check_output([executable, f'@{argument_file}'], text=True)
        rows = [line.split('\t')[1] for line in output.splitlines() if line.startswith('UNIT\t')]
    assert len(rows) == len(cases)
    for case, expected in zip(cases, rows):
        if case['op'] == 'mm':
            case['expected_bits'] = expected.zfill(16)
        else:
            case['expected'] = int(expected)
    result = {'java_commit': PIN, 'cases': cases}
    (ROOT/'crates/ofdrw/tests/fixtures/canvas-units-java.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf-8')
    print('Generated Java unit conversion cases:', len(cases))


if __name__ == '__main__':
    main()
