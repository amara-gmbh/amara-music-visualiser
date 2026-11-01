#!/usr/bin/env python3
"""Fetch an optional, upstream-hosted font after reviewing its licences."""
from pathlib import Path
import hashlib
import os
import tempfile
import urllib.request

URL = "https://api.github.com/repos/ryanoasis/nerd-fonts/git/blobs/fb7c2ab39f128f52269e26cf3eaaa11082ceec8d"
SHA256 = "1c680e8cde9fcf8b88a5605ce8d1fb94dd3fb15841f7ca7bf4c55664855e5611"
SIZE = 2571596
DEST = Path(__file__).resolve().parents[1] / "assets/fonts/JetBrainsMonoNerdFont-Regular.ttf"

def main():
    if DEST.is_symlink() or DEST.exists():
        if not DEST.is_file() or DEST.is_symlink():
            raise SystemExit("Destination is not a regular non-symlink file; refusing to replace it.")
        if hashlib.sha256(DEST.read_bytes()).hexdigest() != SHA256:
            raise SystemExit("An existing font has a different SHA-256; refusing to replace it.")
        print("Existing font matches the pinned SHA-256.")
        return
    request = urllib.request.Request(URL, headers={"Accept": "application/vnd.github.raw+json", "User-Agent": "amaraMusicVisualiser-font-fetch"})
    with urllib.request.urlopen(request, timeout=30) as response:
        content = response.read(SIZE + 1)
    if len(content) != SIZE or hashlib.sha256(content).hexdigest() != SHA256:
        raise SystemExit("Upstream font failed size or SHA-256 verification; nothing installed.")
    DEST.parent.mkdir(parents=True, exist_ok=True)
    tmp_name = None
    try:
        with tempfile.NamedTemporaryFile(dir=DEST.parent, prefix=".amara-font-", delete=False) as temp:
            tmp_name = temp.name
            temp.write(content)
            temp.flush()
            os.fsync(temp.fileno())
        os.chmod(tmp_name, 0o644)
        os.link(tmp_name, DEST)  # Atomic and refuses to overwrite an existing file.
    finally:
        if tmp_name is not None:
            os.unlink(tmp_name)
    print("Downloaded pinned font from upstream; SHA-256 verified.")

if __name__ == "__main__":
    main()
