"""Check that Python, Go, and Node.js declare the current C ABI."""

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUST = (ROOT / "crates/ofdrw-ffi/src/lib.rs").read_text()
NODE = (ROOT / "bindings/node/index.js").read_text()
PYTHON = (ROOT / "bindings/python/ofdrw_rs/__init__.py").read_text()
GO = (ROOT / "bindings/go/ofdrw/ofdrw.go").read_text()

# Node.js uses the JSON text-page writer and richer signature/SES entry points.
NODE_ALIASES = {
    "ofd_add_ses_signature": "ofd_add_ses_signature_with_distids",
    "ofd_add_text_page": "ofd_write_text_page_json",
    "ofd_add_text_page_with_font": "ofd_write_text_page_json",
    "ofd_append_page_text_code_json": "ofd_edit_text_code_json",
    "ofd_append_text_page": "ofd_write_text_page_json",
    "ofd_append_text_page_with_font": "ofd_write_text_page_json",
    "ofd_insert_text_page": "ofd_write_text_page_json",
    "ofd_set_page_text_code": "ofd_edit_text_code_json",
    "ofd_verify_signature_with_direct_trust_anchor_json":
        "ofd_verify_signature_with_direct_trust_anchor_and_distids_json",
    "ofd_verify_signature_with_ordered_sm2_chain_json":
        "ofd_verify_signature_with_ordered_sm2_chain_and_distids_json",
}
GO_ALIASES = {"ofd_add_ses_signature": "ofd_add_ses_signature_with_distids"}

exports = set(re.findall(r'pub (?:unsafe )?extern "C" fn (ofd_\w+)', RUST))
python_cdef = PYTHON.split("_ffi.cdef(", 1)[1].split('"""', 2)[1]
bindings = {
    "Python": (set(re.findall(r"\b(ofd_\w+)\s*\(", python_cdef)), {}),
    "Go": (set(re.findall(r'\{"(ofd_\w+)",\s*&api\.', GO)), GO_ALIASES),
    "Node.js": (
        set(re.findall(r"library\.func\('[^']*\b(ofd_\w+)\s*\(", NODE)),
        NODE_ALIASES,
    ),
}
failed = False
for language, (symbols, aliases) in bindings.items():
    problems = (
        ("missing bindings", sorted(exports - symbols - aliases.keys())),
        ("stale bindings", sorted(symbols - exports)),
        ("stale alias exclusions", sorted(aliases.keys() - exports)),
        ("redundant alias exclusions", sorted(aliases.keys() & symbols)),
        ("missing alias replacements", sorted(set(aliases.values()) - symbols)),
    )
    for label, values in problems:
        if values:
            print(f"{language} {label}: {', '.join(values)}", file=sys.stderr)
            failed = True
    if not any(values for _, values in problems):
        print(f"{language} declares {len(exports)} C exports ({len(aliases)} aliases)")

if failed:
    sys.exit(1)
