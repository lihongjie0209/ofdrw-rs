"""Check that the public C header declares every Rust C ABI export."""

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUST = (ROOT / "crates/ofdrw-ffi/src/lib.rs").read_text()
HEADER = (ROOT / "include/ofdrw.h").read_text()

exports = set(re.findall(r'pub (?:unsafe )?extern "C" fn (ofd_\w+)', RUST))
declarations = set(re.findall(r"\b(ofd_\w+)\s*\(", HEADER))
missing = sorted(exports - declarations)
stale = sorted(declarations - exports)
if missing or stale:
    if missing:
        print("Missing C declarations:", ", ".join(missing), file=sys.stderr)
    if stale:
        print("Stale C declarations:", ", ".join(stale), file=sys.stderr)
    sys.exit(1)

print(f"C header matches all {len(exports)} Rust exports")
