"""Execute pinned Java font shorthand parsing with independent FontSettings."""
import itertools
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = r'''
import java.nio.file.*;
import java.nio.charset.StandardCharsets;
import java.lang.reflect.*;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.element.canvas.*;
import org.ofdrw.font.Font;
public class CanvasFontStyleReference {
    public static void main(String[] args) throws Exception {
        for (boolean repair : new boolean[]{false,true}) {
        Path destination = repair ? Paths.get(args[1]).resolveSibling("reference-restored.ofd") : Paths.get(args[1]);
        try (OFDDoc doc = new OFDDoc(destination)) {
            VirtualPage page = new VirtualPage(40d,30d);
            Canvas canvas = new Canvas(30d,20d);
            canvas.setPosition(Position.Absolute).setX(0d).setY(0d).setBorder(0d);
            canvas.setDrawer(c -> {
                try {
                    Method parse = DrawContext.class.getDeclaredMethod("fontStyleToSetting", String.class, FontSetting.class);
                    parse.setAccessible(true);
                    if (!repair) for (String row : Files.readAllLines(Paths.get(args[0]), StandardCharsets.UTF_8)) {
                        String[] fields = row.split("\t", -1);
                        c.PPM = Double.parseDouble(fields[1]);
                        FontSetting fs = new FontSetting(9, new Font("Base","Base"))
                            .setFontWeight(Integer.parseInt(fields[2])).setItalic(Boolean.parseBoolean(fields[3]));
                        parse.invoke(c, fields[0], fs);
                        System.out.println("FONT\t"+fs.getFont().getName()+"\t"+fs.getFontSize()+"\t"+fs.getFontWeight()+"\t"+fs.isItalic());
                    }
                    c.PPM = 3.78;
                    c.addFont("CanvasTest", Paths.get(args[2]));
                    FontSetting setting = new FontSetting(4,new Font("CanvasTest",Paths.get(args[2])))
                        .setFontWeight(600).setItalic(true).setLetterSpacing(0.25);
                    String css = "italic bold 12px CanvasTest";
                    if (repair) {
                        parse.invoke(c,css,setting);
                        // Restore only the file lost by ResManager.getFont; keep parsed settings.
                        setting.setFont(new Font("CanvasTest",Paths.get(args[2])));
                        c.setFont(setting);
                    } else { c.setFont(setting); c.font = css; }
                    c.fillText("WiA",2,12);
                    if (!repair) System.out.println("LOST_FONT_FILE\t"+(c.getFont().getFont().getFontFile()==null));
                } catch (ReflectiveOperationException error) { throw new RuntimeException(error); }
            });
            page.add(canvas); doc.addVPage(page);
        }
        }
    }
}
'''


def main():
    revision = subprocess.check_output(['git', '-C', str(JAVA), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != PIN:
        raise RuntimeError('Java source is not at the reference pin')
    prefixes = ['', 'italic', 'normal', 'italic normal', 'normal normal']
    prefixes += [style + weight for style in ['', 'italic ', 'normal ']
                 for weight in ['bold', 'bolder', 'lighter', *map(str, range(100, 1000, 100))]]
    cases = []
    for prefix, size, family, ppm, base in itertools.product(
        prefixes, ['12px', '3.17mm', '4'], ['CanvasTest', '宋体'], [3.78, 4], [(400, False), (600, True)]
    ):
        cases.append({'input': f'{prefix} {size} {family}'.strip(), 'pixels_per_mm': ppm,
                      'base_weight': base[0], 'base_italic': base[1]})
    cases += [{'input': 'BOLD 12px CanvasTest', 'pixels_per_mm': 3.78, 'base_weight': 600, 'base_italic': True},
              {'input': 'italic LIGHTER 3.17mm 宋体', 'pixels_per_mm': 4, 'base_weight': 400, 'base_italic': False}]
    defects = [{'input': value, 'pixels_per_mm': 3.78, 'base_weight': 400, 'base_italic': False}
               for value in ['italic bold 12px Times New Roman', "italic bold 12px 'Times New Roman'",
                             'bold italic 12px CanvasTest', 'ITALIC BOLD 12px CanvasTest']]
    requests_all = cases + defects
    classpath = os.pathsep.join(map(str, [*sorted(JAVA.glob('*/target/classes')),
                                          *sorted((Path.home()/'.m2/repository').rglob('*.jar'))]))
    with tempfile.TemporaryDirectory(prefix='ofdrw-font-style-') as temporary:
        root = Path(temporary)
        (root/'CanvasFontStyleReference.java').write_text(SOURCE, encoding='utf-8')
        requests = root/'requests.tsv'
        requests.write_text('\n'.join(f"{c['input']}\t{c['pixels_per_mm']}\t{c['base_weight']}\t{str(c['base_italic']).lower()}" for c in requests_all), encoding='utf-8')
        arguments = [('-proc:none', '-cp', classpath, str(root/'CanvasFontStyleReference.java')),
                     ('-Djava.awt.headless=true', '-cp', str(root)+os.pathsep+classpath,
                      'CanvasFontStyleReference', str(requests), str(root/'reference.ofd'),
                      str(ROOT/'crates/ofdrw/tests/fixtures/dejavu-sans.ttf'))]
        output = ''
        for executable, args in zip(['javac','java'], arguments):
            argument_file = root/f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg, ensure_ascii=False) for arg in args), encoding='utf-8')
            output = subprocess.check_output([executable, f'@{argument_file}'], text=True)
        (ROOT/'crates/ofdrw/tests/fixtures/canvas-font-style-java.ofd').write_bytes((root/'reference.ofd').read_bytes())
        (ROOT/'crates/ofdrw/tests/fixtures/canvas-font-style-java-restored-font.ofd').write_bytes((root/'reference-restored.ofd').read_bytes())
        assert 'LOST_FONT_FILE\ttrue' in output, 'Java cached-font file lookup behavior changed'
        rows = [line.split('\t')[1:] for line in output.splitlines() if line.startswith('FONT\t')]
        assert len(rows) == len(requests_all), (len(rows),len(requests_all))
    for case, (family, size, weight, italic) in zip(requests_all, rows):
        case['expected'] = {'family': family, 'size': float(size), 'weight': int(weight), 'italic': italic=='true'}
    result = {'java_commit': PIN, 'cases': cases, 'known_java_misparses': defects}
    text = json.dumps({'java_commit': PIN, 'known_java_misparses': defects, 'font_lookup_loses_embedded_file': True,
                       'writer_correction': 'Restore the embedded font file after parsing, without changing size, weight, italic or spacing.', 'cases': []}, indent=2, ensure_ascii=False)
    text = text.replace('"cases": []', '"cases": [\n'+',\n'.join('    '+json.dumps(c, ensure_ascii=False) for c in cases)+'\n  ]')
    (ROOT/'crates/ofdrw/tests/fixtures/canvas-font-style-java.json').write_text(text+'\n', encoding='utf-8')
    print('Generated font style cases:', len(result['cases']))


if __name__ == '__main__':
    main()
