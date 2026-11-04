"""Generate Canvas color references by executing the pinned Java NamedColor."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = ROOT.parent
PIN = "84e9eb260ba6fd351ecccf08a01a40305d4af635"
SOURCE = JAVA / "ofdrw-layout/src/main/java/org/ofdrw/layout/element/canvas/NamedColor.java"
HARNESS = '''
import org.ofdrw.layout.element.canvas.NamedColor;
import java.util.Arrays;
public class CanvasColorReference {
    public static void main(String[] args) {
        for (String color : args) {
            try { System.out.println(Arrays.toString(NamedColor.rgb(color))); }
            catch (RuntimeException error) { System.out.println("null"); }
        }
    }
}
'''


def main():
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=JAVA, text=True).strip()
    if head != PIN:
        raise RuntimeError("Java reference commit differs from the recorded pin")
    names = re.findall(r'case "([a-z]+)":', SOURCE.read_text(encoding="utf-8"))
    inputs = [value for name in names for value in (name, name.upper(), "  " + name + "\t")]
    inputs += ["#123", "#abc", "#ABCDEF", "#000000", "#ffffff", "0x123456", "0Xabcdef",
               "16711680", "0777", "-1", "-2147483648", "+0x123456", "#1234567",
               "rgb(0, 128,255)", "RGB(255,0,127)", "rgba(1,2,3,0)", "rgba(1,2,3,1)",
               "rgba(1,2,3,0.5)", "rgba(1,2,3,0.999)", "rgba(1,2,3,1e-1)",
               "", "  ", "unknown", "transparent", "#ggg", "#ffffffff", "2147483648",
               "0x", "08", "rgb()", "rgba(1,2,3)", "rgb(1,2,bad)"]
    with tempfile.TemporaryDirectory(prefix="ofdrw-canvas-color-") as temporary:
        harness = Path(temporary) / "CanvasColorReference.java"
        harness.write_text(HARNESS, encoding="utf-8")
        subprocess.run(["javac", "-encoding", "UTF-8", "-d", temporary, str(SOURCE), str(harness)], check=True)
        output = subprocess.check_output(["java", "-Djava.awt.headless=true", "-cp", temporary,
                                          "CanvasColorReference", *inputs], text=True)
    values = output.splitlines()
    assert len(values) == len(inputs)
    result = {"java_commit": PIN, "source_sha256": hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
              "cases": [{"input": value, "expected": json.loads(expected)}
                        for value, expected in zip(inputs, values)]}
    destination = ROOT / "crates/ofdrw/tests/fixtures/canvas-color-java.json"
    destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print("Generated Java color cases:", len(inputs))


if __name__ == "__main__":
    main()
