"""Generate gradient canvas OFDs using the pinned, read-only Java reference."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = '84e9eb260ba6fd351ecccf08a01a40305d4af635'
CASES = ['axial-fill', 'radial-fill', 'stroke-save', 'alpha-clip', 'text-fill', 'transform-fill']
SOURCE = r'''
import java.nio.file.*;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.element.canvas.*;
import org.ofdrw.font.Font;
public class CanvasGradientReference {
    static CanvasGradient axial(DrawContext c) {
        CanvasGradient g = c.createLinearGradient(0, 0, 22, 0);
        g.addColorStop(0, "#FF0000"); g.addColorStop(0.5, "#FFFF00");
        g.addColorStop(1, "#0000FF"); return g;
    }
    static void write(Path root, String name, Font font) throws Exception {
        try (OFDDoc doc = new OFDDoc(root.resolve(name + ".ofd"))) {
            VirtualPage page = new VirtualPage(40d, 30d);
            Canvas canvas = new Canvas(24d, 20d);
            canvas.setPosition(Position.Absolute).setX(4d).setY(3d).setBorder(0d);
            canvas.setDrawer(c -> {
                c.fillStyle = axial(c);
                switch (name) {
                case "axial-fill": c.fillRect(2, 2, 18, 12); break;
                case "radial-fill":
                    CanvasRadialGradient r = c.createRadialGradient(11, 8, 0, 11, 8, 9);
                    r.addColorStop(0, "#FF0000"); r.addColorStop(0.5, "#FFFF00");
                    r.addColorStop(1, "#0000FF"); c.fillStyle = r;
                    c.fillRect(2, 2, 18, 12); break;
                case "stroke-save":
                case "stroke-save-restored-width":
                    c.fillStyle = null;
                    c.strokeStyle = axial(c);
                    c.setLineWidth(1.2).rect(2, 2, 18, 10).save();
                    c.setStrokeColor(0, 200, 0).stroke();
                    c.restore();
                    // Pinned CanvasState.clone loses drawParam on the saved state.
                    if (name.equals("stroke-save-restored-width")) c.setLineWidth(1.2);
                    c.translate(0, 3).stroke(); break;
                case "alpha-clip":
                    c.rect(3, 3, 12, 10).clip().beginPath().setGlobalAlpha(0.5);
                    c.fillRect(0, 0, 22, 18); break;
                case "text-fill":
                    c.setFont(new FontSetting(4, font)).fillText("Gradient", 2, 12); break;
                case "transform-fill":
                    c.transform(1, 0.2, 0.4, 1, 2, 1).fillRect(2, 2, 14, 10); break;
                }
            });
            page.add(canvas); doc.addVPage(page);
        }
    }
    public static void main(String[] args) throws Exception {
        Path root = Paths.get(args[0]);
        Font font = new Font("CanvasTest", Paths.get(args[1]));
        for (String name : new String[]{"axial-fill", "radial-fill", "stroke-save", "alpha-clip", "text-fill", "transform-fill", "stroke-save-restored-width"}) {
            write(root, name, font);
        }
    }
}
'''


def main():
    revision = subprocess.check_output(['git', '-C', str(JAVA), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != PIN:
        raise SystemExit('Check out the pinned Java reference first')
    if not (JAVA / 'ofdrw-layout/target/classes').is_dir():
        raise SystemExit('Build the sibling Java layout module first')
    classpath = os.pathsep.join(map(str, [*sorted(JAVA.glob('*/target/classes')),
                                         *sorted((Path.home() / '.m2/repository').rglob('*.jar'))]))
    destination = ROOT / 'crates/ofdrw/tests/fixtures/canvas-gradient-java'
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='ofdrw-canvas-gradient-') as temp:
        root = Path(temp)
        source = root / 'CanvasGradientReference.java'
        source.write_text(SOURCE)
        for executable, args in [
            ('javac', ['-proc:none', '-cp', classpath, str(source)]),
            ('java', ['-Djava.awt.headless=true', '-cp', str(root) + os.pathsep + classpath,
                      'CanvasGradientReference', str(root),
                      str(ROOT / 'crates/ofdrw/tests/fixtures/dejavu-sans.ttf')]),
        ]:
            argument_file = root / f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in args) + '\n')
            subprocess.run([executable, f'@{argument_file}'], check=True)
        for name in CASES + ['stroke-save-restored-width']:
            (destination / f'{name}.ofd').write_bytes((root / f'{name}.ofd').read_bytes())
    (destination / 'reference.json').write_text(json.dumps({'java_revision': PIN, 'cases': CASES,
        'reference_overrides': {'stroke-save': 'stroke-save-restored-width.ofd'},
        'known_java_defects': ['CanvasState.clone leaves saved drawParam empty; the override reapplies the known 1.2 mm line width after restore. Raw output is retained.']}, indent=2) + '\n')


if __name__ == '__main__':
    main()
