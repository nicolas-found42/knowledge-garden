#!/usr/bin/env python3
"""Refresh the vendored SDK only from the exact installed source closure lock."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import platform
import re
import shlex
import shutil
import subprocess
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
VENDOR = HERE.parent / "vendor/gmime"


def prefix(formula):
    return Path(subprocess.check_output(["brew", "--prefix", formula], text=True).strip()).resolve()


def refresh():
    lock_path = HERE / "gmime-runtime-lock.json"
    lock = json.loads(lock_path.read_text())
    if platform.machine() != lock["architecture"] or subprocess.check_output(["pkg-config", "--modversion", "gmime-3.0"], text=True).strip() != lock["gmime_version"]:
        raise ValueError("The installed architecture/version differs from the reviewed source lock.")
    with tempfile.TemporaryDirectory(prefix="gmime-refresh-") as temporary:
        work = Path(temporary).resolve()
        probe = work / "probe"
        flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "gmime-3.0"], text=True))
        subprocess.run(["cc", "-O2", str(HERE / "gmime_unit.c"), *flags, "-o", str(probe)], check=True)
        queue, dependencies = [probe], {}
        while queue:
            path = queue.pop().resolve()
            if path in dependencies:
                continue
            dependencies[path] = []
            for line in subprocess.check_output(["otool", "-L", str(path)], text=True).splitlines()[1:]:
                name = line.strip().split(" ", 1)[0]
                if name.startswith("/opt/homebrew/"):
                    target = Path(name).resolve()
                    if target != path:
                        dependencies[path].append((name, target))
                        queue.append(target)
        libraries = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in dependencies if path != probe}
        if libraries != lock["dylibs"]:
            raise ValueError("Installed libraries differ from the reviewed source closure lock; review a dependency update first.")
        stage = work / "distribution"
        runtime, licenses = stage / "runtime", stage / "licenses"
        runtime.mkdir(parents=True)
        licenses.mkdir()
        for path, linked in dependencies.items():
            if path == probe:
                continue
            destination = runtime / path.name
            shutil.copy2(path, destination)
            destination.chmod(0o755)
            changes = ["install_name_tool", "-id", "@loader_path/" + path.name]
            for original, target in linked:
                changes += ["-change", original, "@loader_path/" + target.name]
            subprocess.run([*changes, str(destination)], check=True)
            subprocess.run(["codesign", "--force", "--sign", "-", str(destination)], check=True)
        for formula, relative, target in [("gmime", "include/gmime-3.0", "gmime-3.0"), ("glib", "include/glib-2.0", "glib-2.0"), ("glib", "lib/glib-2.0/include", "glib-config")]:
            shutil.copytree(prefix(formula) / relative, stage / "include" / target)
        shutil.copy2(prefix("gettext") / "include/libintl.h", stage / "include/libintl.h")
        origins = []
        for formula in ["gmime", "glib", "gettext", "gpgme", "libassuan", "libgpg-error", "libidn2", "libunistring", "pcre2"]:
            installed = prefix(formula)
            for pattern in ["COPYING*", "LGPL*.txt"]:
                for source in installed.glob(pattern):
                    if source.is_file():
                        shutil.copy2(source, licenses / f"{formula}-{source.name}")
            formula_text = (installed / ".brew" / f"{formula}.rb").read_text()
            url = re.search(r'^  url "([^"]+)"', formula_text, re.M)
            digest = re.search(r'^  sha256 "([^"]+)"', formula_text, re.M)
            origins.append({"name": formula, "version": installed.name,
                            "formula_sha256": hashlib.sha256(formula_text.encode()).hexdigest(),
                            "source_url": url.group(1) if url else None,
                            "source_sha256": digest.group(1) if digest else None})
        shutil.copy2(lock_path, licenses / lock_path.name)
        (licenses / "NOTICE.txt").write_text("GMime and its pinned dynamic dependency closure include LGPL, GPL and BSD licensed components. Component licenses are included here. Exact versions, source URLs, source hashes and packaged file hashes are recorded in gmime-distribution-manifest.json. These libraries are dynamically linked; no Homebrew installation is required to build or run the packaged helper.\n")
        files = {str(path.relative_to(stage)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(stage.rglob("*")) if path.is_file()}
        VENDOR.mkdir(parents=True, exist_ok=True)
        archive = VENDOR / f"gmime-{lock['gmime_version']}-{lock['architecture']}.tar.gz"
        with archive.open("wb") as output, gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed, tarfile.open(fileobj=compressed, mode="w", format=tarfile.GNU_FORMAT) as package:
            for name in files:
                data = (stage / name).read_bytes()
                member = tarfile.TarInfo(name)
                member.size, member.mtime = len(data), 0
                member.mode = 0o755 if name.startswith("runtime/") else 0o644
                package.addfile(member, io.BytesIO(data))
        manifest = {"format": 1, "architecture": lock["architecture"], "gmime_version": lock["gmime_version"],
                    "archive": archive.name, "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
                    "files": files, "origins": origins,
                    "installed_closure_lock_sha256": hashlib.sha256(lock_path.read_bytes()).hexdigest()}
        (VENDOR / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(f"Packaged {len(files)} files, {archive.stat().st_size} compressed bytes: {manifest['archive_sha256']}")


if __name__ == "__main__":
    refresh()
