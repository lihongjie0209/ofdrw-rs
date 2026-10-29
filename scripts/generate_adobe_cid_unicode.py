"""Vendor pinned Adobe CID-to-Unicode resources, retaining all UTF-16 targets."""
import argparse
import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import io
import json
from pathlib import Path
import zipfile

from generate_adobe_cmaps import api, blob_hash

COMMIT = "2dd5e53fb74a01718b9dfd448a0d1cce6fff2aa5"
REPOSITORY = "adobe-type-tools/mapping-resources-pdf"


def generate(cache, target):
    tree = api(f"git/trees/{COMMIT}?recursive=1", repository=REPOSITORY)
    assert not tree.get("truncated")
    names = [f"Adobe-{ordering}-UCS2" for ordering in ["CNS1", "GB1", "Japan1", "KR", "Korea1"]]
    entries = [entry for entry in tree["tree"] if entry["path"] in [f"pdf2unicode/{name}" for name in names]]
    assert len(entries) == 5

    def fetch(entry):
        path = cache / entry["path"]
        data = path.read_bytes() if path.exists() else b""
        if blob_hash(data) != entry["sha"]:
            response = api(f"contents/{entry['path']}?ref={COMMIT}", repository=REPOSITORY)
            data = base64.b64decode(response["content"], validate=False)
            assert response["sha"] == entry["sha"]
            assert blob_hash(data) == entry["sha"]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        return entry, data

    with ThreadPoolExecutor(max_workers=5) as executor:
        resources = list(executor.map(fetch, entries))
    license_entry = next(entry for entry in tree["tree"] if entry["path"] == "LICENSE.txt")
    _, license_bytes = fetch(license_entry)
    records = []
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        for entry, data in sorted(resources, key=lambda item: item[0]["path"]):
            name = entry["path"].split("/")[-1]
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data, compresslevel=9)
            records.append({"name": name, "ordering": name.removeprefix("Adobe-").removesuffix("-UCS2"),
                            "path": entry["path"], "bytes": len(data), "git_blob_sha1": entry["sha"],
                            "sha256": hashlib.sha256(data).hexdigest()})
    target.mkdir(parents=True, exist_ok=True)
    payload = buffer.getvalue()
    (target / "unicode.zip").write_bytes(payload)
    (target / "manifest.json").write_text(json.dumps({"repository": REPOSITORY, "commit": COMMIT,
        "archive_sha256": hashlib.sha256(payload).hexdigest(), "resources": records}, indent=2) + "\n")
    (target / "NOTICE").write_text(f"Adobe CID-to-Unicode mapping resources\nSource: https://github.com/{REPOSITORY}/tree/{COMMIT}\n"
        "Unmodified pdf2unicode source files, compressed for inclusion in the native library.\n"
        "Each archived resource retains its original copyright and license header.\n\n" + license_bytes.decode())
    print(f"Packaged {len(resources)} CID-to-Unicode resources: {sum(len(data) for _, data in resources)} -> {len(payload)} bytes")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, default=Path("/tmp/ofdrw-adobe-cid-unicode-sources"))
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "crates/ofdrw/assets/cid_unicode")
    args = parser.parse_args()
    generate(args.cache, args.output)
