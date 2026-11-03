#!/usr/bin/env python3
"""Build the MIME helper against the verified, bundled GMime SDK and runtime."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
VENDOR = HERE.parent / "vendor" / "gmime"
MAX_ARCHIVE_BYTES = 16 * 1024 * 1024
MAX_EXPANDED_BYTES = 32 * 1024 * 1024


def unpack_distribution(archive, manifest, destination):
    """Authenticate the complete archive and each regular member before use."""
    if archive.stat().st_size > MAX_ARCHIVE_BYTES:
        raise ValueError("GMime distribution archive exceeds its size limit.")
    with archive.open("rb") as source:
        if hashlib.file_digest(source, "sha256").hexdigest() != manifest["archive_sha256"]:
            raise ValueError("GMime distribution archive failed integrity verification.")
    expected = manifest["files"]
    seen = set()
    expanded = 0
    with tarfile.open(archive, "r:gz") as source:
        for member in source:
            name = PurePosixPath(member.name)
            if (not member.isfile() or not name.parts or name.parts[0] not in {"runtime", "include", "licenses"}
                    or name.is_absolute() or ".." in name.parts or "\\" in member.name
                    or str(name) != member.name or member.name in seen
                    or member.name not in expected):
                raise ValueError("GMime distribution contains an unexpected or unsafe member.")
            expanded += member.size
            if member.size < 0 or expanded > MAX_EXPANDED_BYTES:
                raise ValueError("GMime distribution exceeds its expanded size limit.")
            target = destination.joinpath(*name.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            digest = hashlib.sha256()
            with source.extractfile(member) as input_file, target.open("xb") as output:
                while chunk := input_file.read(64 * 1024):
                    output.write(chunk)
                    digest.update(chunk)
            if digest.hexdigest() != expected[member.name]:
                raise ValueError("GMime distribution member failed integrity verification.")
            target.chmod(0o755 if name.parts[0] == "runtime" else 0o644)
            seen.add(member.name)
    if seen != set(expected):
        raise ValueError("GMime distribution is incomplete.")


def build(output):
    lock = json.loads((HERE / "gmime-runtime-lock.json").read_text())
    manifest = json.loads((VENDOR / "manifest.json").read_text())
    if hashlib.sha256((HERE / "gmime-runtime-lock.json").read_bytes()).hexdigest() != manifest["installed_closure_lock_sha256"]:
        raise ValueError("GMime distribution provenance differs from its source closure lock.")
    if manifest["format"] != 1 or manifest["gmime_version"] != lock["gmime_version"]:
        raise ValueError("GMime distribution version differs from its source lock.")
    if platform.machine() != manifest["architecture"] or manifest["architecture"] != lock["architecture"]:
        raise ValueError("The pinned GMime runtime is for Apple Silicon.")
    archive = VENDOR / manifest["archive"]
    if archive.parent != VENDOR or archive.name != manifest["archive"]:
        raise ValueError("Invalid GMime distribution archive path.")
    with tempfile.TemporaryDirectory(prefix="gmime-build-") as temporary:
        work = Path(temporary)
        unpack_distribution(archive, manifest, work)
        runtime = work / "runtime"
        libraries = {path.name for path in runtime.iterdir()}
        if libraries != set(lock["dylibs"]):
            raise ValueError("GMime distribution does not match the locked library set.")
        for library in runtime.iterdir():
            if subprocess.check_output(["lipo", "-archs", str(library)], text=True).strip() != "arm64":
                raise ValueError("GMime distribution contains an unexpected architecture.")
            for line in subprocess.check_output(["otool", "-L", str(library)], text=True).splitlines()[1:]:
                dependency = line.strip().split(" ", 1)[0]
                if dependency.startswith(("/usr/lib/", "/System/Library/")):
                    continue
                if dependency.startswith("@loader_path/") and dependency.removeprefix("@loader_path/") in libraries:
                    continue
                raise ValueError("GMime distribution has an unbundled dynamic dependency.")
        executable = work / "kg-mime-unit"
        include = work / "include"
        direct = ["libgmime-3.0.0.dylib", "libgio-2.0.0.dylib", "libgobject-2.0.0.dylib", "libglib-2.0.0.dylib", "libintl.8.dylib"]
        subprocess.run(["cc", "-O2", "-Wl,-headerpad_max_install_names",
                        "-I" + str(include / "gmime-3.0"), "-I" + str(include / "glib-2.0"),
                        "-I" + str(include / "glib-config"), "-I" + str(include),
                        str(HERE / "gmime_unit.c"), *[str(runtime / name) for name in direct],
                        "-o", str(executable)], check=True)
        changes = ["install_name_tool"]
        for name in direct:
            changes += ["-change", "@loader_path/" + name, "@rpath/" + name]
        for path in ["@executable_path/../Resources/container-runtime", "@executable_path/../../resources/container-runtime", "@executable_path/container-runtime"]:
            changes += ["-add_rpath", path]
        subprocess.run([*changes, str(executable)], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", str(executable)], check=True)
        output.mkdir(parents=True, exist_ok=True)
        for source, destination in [(runtime, output / "container-runtime"), (work / "licenses", output / "container-licenses")]:
            destination.mkdir(exist_ok=True)
            for item in source.iterdir():
                target = destination / item.name
                target.unlink(missing_ok=True)
                shutil.copy2(item, target)
        target = output / "kg-mime-unit"
        target.unlink(missing_ok=True)
        shutil.copy2(executable, target)
        shutil.copy2(VENDOR / "manifest.json", output / "container-licenses/gmime-distribution-manifest.json")
    print(output)


if __name__ == "__main__":
    build(Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else HERE / "generated-gmime")
