"""Execute pinned DrawContext.createPattern and retain independent Java OFDs."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = '84e9eb260ba6fd351ecccf08a01a40305d4af635'
CASES = [
    {'name': 'repeat-natural', 'repetition': 'repeat', 'ppm': 3.78},
    {'name': 'empty-natural', 'repetition': '', 'ppm': 4},
    {'name': 'column-custom', 'repetition': 'COLUMN', 'ppm': 4, 'size': [4, 3]},
    {'name': 'row-transform', 'repetition': 'row', 'ppm': 4, 'size': [4, 3], 'transform': [1, .25, .2, 1, 1, 2]},
    {'name': 'row-column-affine', 'repetition': 'Row-CoLuMn', 'ppm': 2, 'size': [4, 3],
     'transform': [.8, .3, -.2, 1, 1, 0], 'canvas_ctm': [1, .1, .2, 1, 0, 0]},
    {'name': 'fractional-size', 'repetition': 'repeat', 'ppm': 4, 'size': [1.2345, .123456]},
    {'name': 'pattern-stroke', 'repetition': 'repeat', 'ppm': 3.78, 'size': [4, 3], 'stroke': True},
    {'name': 'mirrored-alpha', 'repetition': 'repeat', 'ppm': 4, 'size': [4, 3],
     'transform': [-1, 0, 0, 1, 5, 0], 'alpha': .5},
]
SOURCE = r'''
import java.nio.file.*;
import javax.xml.parsers.DocumentBuilderFactory;
import org.w3c.dom.*;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.element.canvas.*;
public class CanvasPatternReference {
    static double[] values(Element scene, String name) {
        String[] fields = scene.getAttribute(name).split(" ");
        double[] result = new double[fields.length];
        for (int i=0;i<fields.length;i++) result[i]=Double.parseDouble(fields[i]);
        return result;
    }
    public static void main(String[] args) throws Exception {
        Path root = Paths.get(args[1]), image = Paths.get(args[2]);
        Element scenes = DocumentBuilderFactory.newInstance().newDocumentBuilder().parse(args[0]).getDocumentElement();
        for (Node node=scenes.getFirstChild();node!=null;node=node.getNextSibling()) {
            if (!(node instanceof Element)) continue;
            Element scene=(Element)node;
            try (OFDDoc doc=new OFDDoc(root.resolve(scene.getAttribute("name")+".ofd"))) {
                VirtualPage page=new VirtualPage(40d,30d);
                Canvas canvas=new Canvas(24d,20d);
                canvas.setPosition(Position.Absolute).setX(4d).setY(3d).setBorder(0d);
                canvas.setDrawer(c -> {
                    c.PPM=Double.parseDouble(scene.getAttribute("ppm"));
                    CanvasPattern pattern=c.createPattern(image,scene.getAttribute("repetition"));
                    if (scene.hasAttribute("size")) {
                        double[] size=values(scene,"size"); pattern.setImageSize(size[0],size[1]);
                    }
                    if (scene.hasAttribute("transform")) pattern.setTransform(values(scene,"transform"));
                    if (scene.hasAttribute("canvas_ctm")) {
                        double[] m=values(scene,"canvas_ctm"); c.setTransform(m[0],m[1],m[2],m[3],m[4],m[5]);
                    }
                    if (scene.hasAttribute("alpha")) c.setGlobalAlpha(Double.parseDouble(scene.getAttribute("alpha")));
                    if (scene.hasAttribute("stroke")) {
                        c.strokeStyle=pattern; c.setLineWidth(1.2).rect(2,2,18,12).stroke();
                    } else { c.fillStyle=pattern; c.fillRect(2,2,18,12); }
                });
                page.add(canvas); doc.addVPage(page);
            }
        }
    }
}
'''


def main():
    revision = subprocess.check_output(['git','-C',str(JAVA),'rev-parse','HEAD'],text=True).strip()
    if revision != PIN:
        raise RuntimeError('Java source is not at the reference pin')
    report = JAVA/'ofdrw-converter/target/surefire-reports/TEST-OFD2IMGTest.xml'
    if not report.exists():
        raise RuntimeError('Build and run pinned Java converter OFD2IMGTest first')
    properties = ET.parse(report).getroot().find('properties')
    classpath = next(node.get('value') for node in properties if node.get('name')=='java.class.path')
    classpath = os.pathsep.join(entry for entry in classpath.split(os.pathsep) if Path(entry).exists() and '/target/test-classes' not in entry)
    destination = ROOT/'crates/ofdrw/tests/fixtures/canvas-pattern-java'
    destination.mkdir(exist_ok=True)
    scenes = ET.Element('scenes')
    for case in CASES:
        ET.SubElement(scenes,'scene',{key:' '.join(map(str,value)) if isinstance(value,list) else str(value)
            for key,value in case.items()})
    with tempfile.TemporaryDirectory(prefix='ofdrw-canvas-pattern-') as temporary:
        root = Path(temporary)
        source = root/'CanvasPatternReference.java'
        source.write_text(SOURCE,encoding='utf-8')
        requests = root/'requests.xml'
        ET.ElementTree(scenes).write(requests,encoding='utf-8')
        arguments = [('-proc:none','-cp',classpath,str(source)),
            ('-Djava.awt.headless=true','-cp',str(root)+os.pathsep+classpath,'CanvasPatternReference',str(requests),str(root),
             str(ROOT/'crates/ofdrw/tests/fixtures/canvas-images-java/source.png'))]
        for executable,args in zip(['javac','java'],arguments):
            argument_file = root/f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in args),encoding='utf-8')
            subprocess.run([executable,f'@{argument_file}'],check=True)
        for case in CASES:
            file = root/(case['name']+'.ofd')
            (destination/file.name).write_bytes(file.read_bytes())
            with zipfile.ZipFile(file) as archive:
                patterns = [node for name in archive.namelist() if name.endswith('.xml')
                    for node in ET.fromstring(archive.read(name)).iter() if node.tag.split('}')[-1]=='Pattern']
                assert len(patterns)==1
                pattern = patterns[0]
                image = next(node for node in pattern.iter() if node.tag.split('}')[-1]=='ImageObject')
                case['expected'] = {'width':float(pattern.get('Width')),'height':float(pattern.get('Height')),
                    'reflect':pattern.get('ReflectMethod','Normal'),'ctm':pattern.get('CTM'),
                    'image_boundary':list(map(float,image.get('Boundary').split())),
                    'image_ctm':list(map(float,image.get('CTM').split()))}
    metadata = {'java_commit':PIN,'cases':CASES,
        'render_scope':'Java DrawContext writer output; comparisons render both OFDs with Rust, not an independent rasterizer.'}
    (destination/'reference.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8')
    print('Generated Java Canvas pattern scenes:',len(CASES))


if __name__=='__main__':
    main()
