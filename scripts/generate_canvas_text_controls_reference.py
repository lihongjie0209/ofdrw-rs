"""Generate independent Java canvas advances for ASCII control characters."""
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
import org.ofdrw.font.Font;
import org.ofdrw.layout.element.canvas.*;
public class CanvasControlsReference {
 public static void main(String[] args) throws Exception {
  FontSetting font=new FontSetting(4,new Font("CanvasTest",Paths.get(args[0])));
  for(int code=0;code<=32;code++) {
   MeasureBody m=TextMeasureTool.measureWithWith("A"+(char)code+"B",font);
   System.out.println("case:"+code+"\t"+m.width+"\t"+Arrays.toString(m.offset));
  }
 }
}
'''


def main():
    revision = subprocess.check_output(['git','-C',str(JAVA),'rev-parse','HEAD'],text=True).strip()
    if revision != PIN:
        raise SystemExit('Check out the pinned Java reference first')
    if not (JAVA/'ofdrw-layout/target/classes').is_dir():
        raise SystemExit('Build the sibling Java layout module first')
    classpath=os.pathsep.join(map(str,[*sorted(JAVA.glob('*/target/classes')),*sorted((Path.home()/'.m2/repository').rglob('*.jar'))]))
    font=ROOT/'crates/ofdrw/tests/fixtures/dejavu-sans.ttf'
    with tempfile.TemporaryDirectory(prefix='ofdrw-canvas-controls-') as temp:
        root=Path(temp)
        (root/'CanvasControlsReference.java').write_text(SOURCE)
        for executable,args in [
            ('javac',['-proc:none','-cp',classpath,str(root/'CanvasControlsReference.java')]),
            ('java',['-cp',os.pathsep.join([str(root),classpath]),'CanvasControlsReference',str(font)]),
        ]:
            argument_file=root/f'{executable}.args'
            argument_file.write_text('\n'.join(json.dumps(arg) for arg in args)+'\n')
            result=subprocess.run([executable,f'@{argument_file}'],check=True,capture_output=True,text=True)
        cases=[]
        for line in result.stdout.splitlines():
            if line.startswith('case:'):
                code,width,offsets=line[5:].split('\t')
                cases.append({'code':int(code),'metrics':{'width':float(width),'first_offset':[0.0,0.0],'offsets':json.loads(offsets)}})
    assert len(cases)==33
    data={'java_revision':PIN,'font_size':4,'cases':cases}
    (ROOT/'crates/ofdrw/tests/fixtures/canvas-text-java/controls.json').write_text(json.dumps(data,indent=2)+'\n')


if __name__=='__main__':
    main()
