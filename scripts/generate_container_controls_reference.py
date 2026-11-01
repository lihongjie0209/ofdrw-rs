"""Generate BR/PageAreaFiller coordinates using the pinned Java OFDRW build.

Build the sibling reference first; this generator never calls Rust.
"""
from pathlib import Path
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = r"""
import java.util.*;
import org.ofdrw.layout.*;
import org.ofdrw.layout.element.*;
import org.ofdrw.layout.engine.*;
public class ContainerControlsReference {
    static Div box(double w, double h, AFloat f, Clear c) {
        return new Div(w,h).setFloat(f).setClear(c);
    }
    static Div a() { return box(4,3,AFloat.left,Clear.none); }
    static Div b() { return box(5,3,AFloat.right,Clear.none); }
    static Div c() { return box(6,3,AFloat.left,Clear.both); }
    static void emit(String name, Div... input) {
        PageLayout layout = new PageLayout(20d,14d).setMargin(2d);
        List<Segment> segments = new SegmentationEngine(layout).process(Arrays.asList(input));
        List<VirtualPage> pages = new StreamingLayoutAnalyzer(layout).analyze(segments);
        for (int page = 0; page < pages.size(); page++) {
            System.out.printf("P\t%s\t%d%n", name, page);
            for (Div div : pages.get(page).getContent()) {
                String label = div.getWidth() == 4d ? "A" : div.getWidth() == 5d ? "B" : "C";
                System.out.printf(Locale.ROOT,"B\t%s\t%d\t%s\t%.9f\t%.9f%n",name,page,label,div.getX(),div.getY());
            }
        }
    }
    public static void main(String[] args) {
        emit("row_break",a(),new BR(),b());
        emit("empty_breaks",new BR(),new BR(),a(),new BR(),new BR(),b(),new BR(),new BR());
        emit("fills",a().setClear(Clear.both),new PageAreaFiller(),new PageAreaFiller(),c(),new PageAreaFiller(),new PageAreaFiller());
        emit("leading_fill",new PageAreaFiller(),a().setClear(Clear.both));
        emit("trailing_fill",a().setClear(Clear.both),new PageAreaFiller());
        emit("full_fill",box(4,10,AFloat.left,Clear.both),new PageAreaFiller());
        emit("fills_and_br",a(),b(),new BR(),new PageAreaFiller(),c());
    }
}
"""

def main():
    revision = subprocess.run(["git", "-C", str(JAVA), "rev-parse", "HEAD"], check=True, capture_output=True, text=True).stdout.strip()
    if revision != PIN:
        raise SystemExit("Check out and build the pinned Java OFDRW reference first")
    if not (JAVA / "ofdrw-layout/target/classes").is_dir():
        raise SystemExit("Build the sibling Java ofdrw-layout module first")
    classpath = os.pathsep.join(str(p) for p in sorted(JAVA.glob("*/target/classes")))
    with tempfile.TemporaryDirectory(prefix="ofdrw-controls-reference-") as directory:
        source = Path(directory) / "ContainerControlsReference.java"
        source.write_text(SOURCE)
        def argfile(name, arguments):
            path = Path(directory) / name
            path.write_text("\n".join('"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"' for v in arguments))
            return "@" + str(path)
        subprocess.run(["javac", argfile("compile.args", ["-proc:none", "-cp", classpath, str(source)])], check=True)
        result = subprocess.run(["java", argfile("run.args", ["-cp", directory + os.pathsep + classpath, "ContainerControlsReference"])], check=True, capture_output=True, text=True)
    path = ROOT / "crates/ofdrw/tests/fixtures/container-controls-java.tsv"
    path.write_text(f"# Java OFDRW revision {revision}; generated independently of Rust.\n"
                    "# P case page; B case page label x y\n" + result.stdout)
    print(path.relative_to(ROOT))

if __name__ == "__main__":
    main()
