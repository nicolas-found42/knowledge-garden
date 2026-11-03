"""Integrity and extraction regressions for the pinned MIME distribution."""
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import provision_gmime as pinned


class DistributionTests(unittest.TestCase):
    def test_checked_in_distribution_verifies_every_file(self):
        manifest = json.loads((pinned.VENDOR / "manifest.json").read_text())
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            pinned.unpack_distribution(pinned.VENDOR / manifest["archive"], manifest, output)
            self.assertEqual(len(manifest["files"]), sum(p.is_file() for p in output.rglob("*")))
            self.assertTrue((output / "licenses/glib-LGPL-2.1-or-later.txt").is_file())

    def test_altered_archive_is_rejected_before_extraction(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / "bad.tar.gz"
            archive.write_bytes(b"changed distribution")
            output = Path(temporary) / "output"
            output.mkdir()
            with self.assertRaisesRegex(ValueError, "integrity"):
                pinned.unpack_distribution(archive, {"archive_sha256": "0" * 64, "files": {}}, output)
            self.assertEqual(list(output.iterdir()), [])

    def test_matching_archive_hash_cannot_authorize_unsafe_members(self):
        for name, kind in [("../escape", tarfile.REGTYPE), ("/absolute", tarfile.REGTYPE), ("runtime/link", tarfile.SYMTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                archive = Path(temporary) / "bad.tar.gz"
                entry = tarfile.TarInfo(name)
                entry.type = kind
                entry.linkname = "../../escape" if kind == tarfile.SYMTYPE else ""
                entry.size = 1 if kind == tarfile.REGTYPE else 0
                with tarfile.open(archive, "w:gz") as output:
                    output.addfile(entry, io.BytesIO(b"x"))
                manifest = {"archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(), "files": {name: hashlib.sha256(b"x").hexdigest()}}
                output = Path(temporary) / "output"
                output.mkdir()
                with self.assertRaisesRegex(ValueError, "unsafe"):
                    pinned.unpack_distribution(archive, manifest, output)
                self.assertFalse((Path(temporary) / "escape").exists())

    def test_missing_and_changed_members_are_rejected(self):
        for files in [{"runtime/absent": "0" * 64}, {"runtime/library": "0" * 64}]:
            with self.subTest(files=files), tempfile.TemporaryDirectory() as temporary:
                archive = Path(temporary) / "bad.tar.gz"
                with tarfile.open(archive, "w:gz") as output:
                    entry = tarfile.TarInfo("runtime/library")
                    entry.size = 1
                    output.addfile(entry, io.BytesIO(b"x"))
                manifest = {"archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(), "files": files}
                output = Path(temporary) / "output"
                output.mkdir()
                with self.assertRaises(ValueError):
                    pinned.unpack_distribution(archive, manifest, output)


if __name__ == "__main__":
    unittest.main()
