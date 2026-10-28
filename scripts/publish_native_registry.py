"""Publish immutable native registry versions, verifying existing asset digests."""

import base64
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
USER_AGENT = "ofdrw-rs release (https://github.com/lihongjie0209/ofdrw-rs)"


def read_url(url):
    try:
        with urlopen(Request(url, headers={"User-Agent": USER_AGENT}), timeout=30) as response:
            return response.read()
    except HTTPError as error:
        if error.code == 404:
            return None
        raise


def checksum(file, algorithm="sha256"):
    with file.open("rb") as source:
        return hashlib.file_digest(source, algorithm)


def require_digest(file, expected, algorithm="sha256"):
    actual = checksum(file, algorithm).hexdigest()
    if actual != expected:
        raise SystemExit(f"Registry already contains different bytes: {file.name}")


def metadata(url):
    data = read_url(url)
    return json.loads(data) if data is not None else None


def verify_pypi(files, published):
    existing = {file["filename"]: file["digests"]["sha256"] for file in (published or {}).get("urls", [])}
    missing = []
    for file in files:
        if file.name in existing:
            require_digest(file, existing[file.name])
        else:
            missing.append(file)
    return missing


def cargo_checksum(version):
    data = read_url("https://index.crates.io/of/dr/ofdrw")
    for line in (data or b"").splitlines():
        item = json.loads(line)
        if item["vers"] == version:
            return item["cksum"]
    return None


def main():
    registry, tag, directory = sys.argv[1:]
    subprocess.run([sys.executable, str(ROOT / "scripts/check_release_version.py"), tag], check=True)
    output = Path(directory).resolve()
    version = tag[1:]
    if registry == "pypi":
        files = sorted(output.glob(f"ofdrw_rs-{version}-*.whl"))
        if len(files) != 6:
            raise SystemExit("Expected six Python wheels")
        url = f"https://pypi.org/pypi/ofdrw-rs/{version}/json"
        missing = verify_pypi(files, metadata(url))
        if missing:
            subprocess.run([sys.executable, "-m", "twine", "upload", "--non-interactive", *map(str, missing)], check=True)
        for _ in range(12):
            if not verify_pypi(files, metadata(url)):
                print(f"Verified PyPI ofdrw-rs {version}")
                return
            time.sleep(5)
    elif registry == "cargo":
        file = output / f"ofdrw-{version}.crate"
        existing = cargo_checksum(version)
        if existing is None:
            subprocess.run(["cargo", "publish", "-p", "ofdrw", "--locked", "--no-verify"], cwd=ROOT, check=True)
        for _ in range(12):
            existing = cargo_checksum(version)
            if existing is not None:
                require_digest(file, existing)
                print(f"Verified crates.io ofdrw {version}")
                return
            time.sleep(5)
    elif registry == "npm":
        file = output / f"ofdrw-rs-{version}.tgz"
        url = f"https://registry.npmjs.org/ofdrw-rs/{version}"
        published = metadata(url)
        if published is None:
            subprocess.run(["npm", "publish", str(file), "--access", "public", "--provenance",
                            "--registry=https://registry.npmjs.org"], check=True)
        for _ in range(12):
            published = metadata(url)
            if published is not None:
                expected = "sha512-" + base64.b64encode(checksum(file, "sha512").digest()).decode()
                if published["dist"]["integrity"] != expected:
                    raise SystemExit("npm already contains a different tarball")
                print(f"Verified npm ofdrw-rs {version}")
                return
            time.sleep(5)
    else:
        raise SystemExit(f"Unsupported registry: {registry}")
    raise SystemExit(f"Published {registry} version is not visible after 60 seconds")


if __name__ == "__main__":
    main()
