"""Publish non-npm distributions as OCI artifacts in GitHub Packages (GHCR)."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
tag, directory = sys.argv[1:]
subprocess.run([sys.executable, str(root / "scripts/check_release_version.py"), tag], check=True)
output = Path(directory).resolve()
repository = os.environ["GITHUB_REPOSITORY"].lower()
owner = repository.split("/")[0]
commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
groups = {
    "python": sorted(output.glob(f"ofdrw_rs-{tag[1:]}-*.whl")),
    "go": [output / f"ofdrw-go-{tag}.zip"],
    "rust": [output / f"ofdrw-{tag[1:]}.crate"],
    "native": sorted(output.glob(f"ofdrw-native-*-{tag}.zip")),
}
for language, files in groups.items():
    expected = 6 if language in {"python", "native"} else 1
    if len(files) != expected or any(not file.is_file() for file in files):
        raise SystemExit(f"Incomplete {language} distribution")
    reference = f"ghcr.io/{owner}/ofdrw-rs-{language}:{tag[1:]}"
    with tempfile.TemporaryDirectory(prefix=f"ofdrw-{language}-") as temporary:
        staging = Path(temporary)
        for file in [*files, root / "LICENSE", root / "NOTICE"]:
            shutil.copy2(file, staging / file.name)
        entries = sorted(staging.iterdir())
        checksums = []
        for file in entries:
            with file.open("rb") as source:
                checksums.append(f"{hashlib.file_digest(source, 'sha256').hexdigest()}  {file.name}")
        (staging / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
        command = [
            "oras", "push", reference,
            "--artifact-type", "application/vnd.ofdrw.library.v1",
            "--annotation", f"org.opencontainers.image.source=https://github.com/{repository}",
            "--annotation", f"org.opencontainers.image.revision={commit}",
            "--annotation", f"org.opencontainers.image.version={tag[1:]}",
            "--annotation", "org.opencontainers.image.licenses=Apache-2.0",
            "--annotation", f"org.opencontainers.image.description=ofdrw-rs {language} library distributions",
            *[file.name for file in sorted(staging.iterdir())],
        ]
        subprocess.run(command, cwd=staging, check=True)
        subprocess.run(["oras", "manifest", "fetch", reference], check=True, stdout=subprocess.DEVNULL)
        downloaded = staging / "downloaded"
        subprocess.run(["oras", "pull", reference, "--output", str(downloaded)], check=True)
        for file in [*entries, staging / "SHA256SUMS"]:
            with file.open("rb") as expected, (downloaded / file.name).open("rb") as actual:
                if hashlib.file_digest(expected, "sha256").digest() != hashlib.file_digest(actual, "sha256").digest():
                    raise SystemExit(f"Registry download differs: {reference}/{file.name}")
    print(f"Published GitHub Package: {reference}")
