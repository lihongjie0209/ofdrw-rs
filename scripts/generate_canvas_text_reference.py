"""Generate canvas direction and measurement references with read-only Java OFDRW."""
from pathlib import Path
import json
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = r'''
import java.nio.file.*;
import java.util.*;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.element.canvas.*;
import org.ofdrw.layout.element.canvas.TextAlign;
import org.ofdrw.font.Font;
public class CanvasTextReference {
    public static void main(String[] args) throws Exception {
        Path output=Paths.get(args[0]);
        Font font=new Font("CanvasTest",Paths.get(args[1]));
        StringBuilder metrics=new StringBuilder("[");
        try (OFDDoc doc=new OFDDoc(output.resolve("directions.ofd"))) {
            VirtualPage page=new VirtualPage(90d,90d);
            Canvas canvas=new Canvas(80d,80d);
            canvas.setPosition(Position.Absolute).setX(4d).setY(3d).setBorder(0d);
            canvas.setDrawer(c -> {
                c.translate(1,1).setGlobalAlpha(0.5);
                for (int rd: new int[]{0,90,180,270}) {
                    for (int cd: new int[]{0,90,180,270}) {
                        TextAlign align=rd==90?TextAlign.center:rd==180?TextAlign.end:rd==270?TextAlign.right:TextAlign.start;
                        FontSetting fs=new FontSetting(4,font).setReadDirection(rd).setCharDirection(cd)
                            .setLetterSpacing(0.25).setTextAlign(align).setFontWeight(cd==90?700:400).setItalic(rd==270);
                        MeasureBody body=TextMeasureTool.measureWithWith("WiA",fs);
                        if (metrics.length()>1) metrics.append(',');
                        metrics.append(String.format(Locale.ROOT,
                            "{\"read_direction\":%d,\"char_direction\":%d,\"align\":\"%s\",\"width\":%s,\"first_offset\":[%s,%s],\"offsets\":%s}",
                            rd,cd,align,body.width,body.firstCharOffsetX,body.firstCharOffsetY,Arrays.toString(body.offset)));
                        c.setFont(fs).fillText("WiA",12+cd/90*20,12+rd/90*20);
                    }
                }
            });
            page.add(canvas); doc.addVPage(page);
        }
        metrics.append(']');
        Files.writeString(output.resolve("metrics.json"),metrics.toString());
        StringBuilder formatting=new StringBuilder("[");
        for (double number:new double[]{1.0625,-1.0625,1.0005,1.0015,0.001953125,0.000000005,0.999999995,-0.999999995,1.2344999999999999,1e30,1e-20}) {
            if (formatting.length()>1) formatting.append(',');
            formatting.append("{\"value\":"+number+",\"formatted\":\""+org.ofdrw.core.basicType.STBase.fmt(number)+"\"}");
        }
        Files.writeString(output.resolve("format.json"),formatting.append(']').toString());
    }
}
'''


def main():
    revision=subprocess.check_output(['git','-C',str(JAVA),'rev-parse','HEAD'],text=True).strip()
    if revision != PIN:
        raise SystemExit('Check out the pinned Java reference first')
    if not (JAVA/'ofdrw-layout/target/classes').is_dir():
        raise SystemExit('Build the sibling Java layout module first')
    classpath=os.pathsep.join(map(str,[*sorted(JAVA.glob('*/target/classes')),*sorted((Path.home()/'.m2/repository').rglob('*.jar'))]))
    destination=ROOT/'crates/ofdrw/tests/fixtures/canvas-text-java'
    destination.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='ofdrw-canvas-text-') as temp:
        root=Path(temp)
        (root/'CanvasTextReference.java').write_text(SOURCE)
        for executable,args in [
            ('javac',['-proc:none','-cp',classpath,str(root/'CanvasTextReference.java')]),
            ('java',['-cp',os.pathsep.join([str(root),classpath]),'CanvasTextReference',str(root),str(ROOT/'crates/ofdrw/tests/fixtures/dejavu-sans.ttf')]),
        ]:
            argument_file=root/f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in args)+'\n')
            subprocess.run([executable,f'@{argument_file}'],check=True)
        (destination/'directions.ofd').write_bytes((root/'directions.ofd').read_bytes())
        data={'java_revision':PIN,'text':'WiA','cases':json.loads((root/'metrics.json').read_text()),'number_format':json.loads((root/'format.json').read_text())}
        (destination/'reference.json').write_text(json.dumps(data,indent=2)+'\n')


if __name__=='__main__':
    main()
