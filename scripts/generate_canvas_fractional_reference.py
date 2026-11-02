"""Reference fractional Canvas metrics from pinned Java and OpenJDK FreeType."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = '84e9eb260ba6fd351ecccf08a01a40305d4af635'
SOURCE = r'''
import java.nio.file.*;
import org.ofdrw.font.Font;
import org.ofdrw.layout.element.canvas.*;
public class CanvasFractionalReference {
    public static void main(String[] args) throws Exception {
        String[] texts = {"WiA", " A ", "\t\n\r", "中A", "A\u0301", "\u0301\u05b0\u093e", "\u05d0\u0628\u0915", "\u0e01\u1100\u1780", "\u200c\u200d\u202e"};
        double[] sizes = {0.01,0.25,0.5,0.99,1,1.001,1.0156249,1.015625,1.2345,
                          12d/3.78,4,4.00001,4.25,7.9999999,12.5,100.123,16383.99,20000.25};
        for (int i=0;i<sizes.length;i++) for (double spacing:new double[]{0,0.25}) {
            FontSetting setting = new FontSetting(sizes[i],new Font("Fractional",Paths.get(args[0])))
                .setLetterSpacing(spacing);
            for (int j=0;j<texts.length;j++) {
                TextMetricsArea m = TextMeasureTool.measureArea(texts[j],setting);
                StringBuilder boxes = new StringBuilder("[");
                for (java.awt.geom.Rectangle2D box : m.charAreas) {
                    if(boxes.length()>1) boxes.append(',');
                    boxes.append("[0,0,").append(box.getWidth()).append(',').append(box.getHeight()).append(']');
                }
                boxes.append(']');
                System.out.println("CASE\t"+sizes[i]+"\t"+spacing+"\t"+j+"\t"+m.width+"\t"+m.height+"\t"+boxes);
            }
        }
    }
}
'''


def main():
    revision = subprocess.check_output(['git','-C',str(JAVA),'rev-parse','HEAD'],text=True).strip()
    if revision != PIN:
        raise RuntimeError('Java source does not match the reference pin')
    classpath = os.pathsep.join(map(str,[*sorted(JAVA.glob('*/target/classes')),
                                       *sorted((Path.home()/'.m2/repository').rglob('*.jar'))]))
    cases = []
    texts = ['WiA',' A ','\t\n\r','中A','A\u0301','\u0301\u05b0\u093e','\u05d0\u0628\u0915','\u0e01\u1100\u1780','\u200c\u200d\u202e']
    with tempfile.TemporaryDirectory(prefix='ofdrw-fractional-') as temporary:
        root = Path(temporary)
        source = root/'CanvasFractionalReference.java'
        source.write_text(SOURCE,encoding='utf-8')
        args = root/'compile.args'
        args.write_text('\n'.join(json.dumps(v) for v in ['-proc:none','-cp',classpath,str(source)]))
        subprocess.run(['javac',f'@{args}'],check=True)
        for font in ['dejavu-sans.ttf','dejavu-serif.ttf','kerning-simhei.ttf','test-name-cff.otf']:
            args = root/'run.args'
            args.write_text('\n'.join(json.dumps(v) for v in ['-Djava.awt.headless=true','-cp',str(root)+os.pathsep+classpath,
                'CanvasFractionalReference',str(ROOT/'crates/ofdrw/tests/fixtures'/font)]))
            output = subprocess.check_output(['java',f'@{args}'],text=True)
            for row in output.splitlines():
                if not row.startswith('CASE\t'): continue
                _,size,spacing,index,width,height,boxes = row.split('\t')
                cases.append({'font':font,'size':float(size),'letter_spacing':float(spacing),
                              'text':texts[int(index)],'width':float(width),'height':float(height),'char_areas':json.loads(boxes)})
    version = subprocess.run(['java','--version'],text=True,capture_output=True,check=True).stdout.splitlines()[0]
    metadata = {'java_commit':PIN,'java_version':version,'cases':[]}
    data = json.dumps(metadata,indent=2).replace('"cases": []','"cases": [\n'+',\n'.join('    '+json.dumps(c,ensure_ascii=True) for c in cases)+'\n  ]')
    (ROOT/'crates/ofdrw/tests/fixtures/canvas-fractional-java.json').write_text(data+'\n',encoding='utf-8')
    print('Generated fractional metric cases:',len(cases))


if __name__=='__main__': main()
