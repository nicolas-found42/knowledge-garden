#!/usr/bin/env python3
"""Build the pinned GMime helper and its local dynamic-library closure."""
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile

here = Path(__file__).resolve().parent
lock = json.loads((here / "gmime-runtime-lock.json").read_text())
if platform.machine() != lock["architecture"]:
    sys.exit("The pinned GMime runtime is for Apple Silicon.")
if subprocess.check_output(["pkg-config", "--modversion", "gmime-3.0"], text=True).strip() != lock["gmime_version"]:
    sys.exit("Install the pinned GMime version before provisioning.")
output = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else here / "generated-gmime"
output.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix="gmime-build-") as temp:
    work = Path(temp).resolve()
    executable = work / "kg-mime-unit"
    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "gmime-3.0"], text=True))
    subprocess.run(["cc", "-O2", "-Wl,-headerpad_max_install_names", str(here / "gmime_unit.c"), *flags, "-o", str(executable)], check=True)
    queue = [executable.resolve()]
    seen = {}
    dependencies = {}
    while queue:
        path = queue.pop().resolve()
        if path in seen:
            continue
        seen[path] = path.name
        dependencies[path] = []
        for line in subprocess.check_output(["otool", "-L", str(path)], text=True).splitlines()[1:]:
            name = line.strip().split(" ", 1)[0]
            if name.startswith("/opt/homebrew/"):
                target = Path(name).resolve()
                if target != path:
                    dependencies[path].append((name, target))
                    queue.append(target)
    dylibs = {path.name: hashlib.file_digest(path.open("rb"), "sha256").hexdigest() for path in seen if path != executable}
    if dylibs != lock["dylibs"]:
        sys.exit("The installed GMime dependency closure does not match gmime-runtime-lock.json.")
    runtime = output / "container-runtime"
    runtime.mkdir(exist_ok=True)
    for path in seen:
        destination = output / "kg-mime-unit" if path == executable else runtime / path.name
        destination.unlink(missing_ok=True)
        shutil.copy2(path, destination)
        destination.chmod(destination.stat().st_mode | 0o200)
    for path in seen:
        dest = output / "kg-mime-unit" if path == executable else runtime / path.name
        args = ["install_name_tool"]
        if path != executable:
            args += ["-id", "@loader_path/" + path.name]
        for original, target in dependencies[path]:
            prefix = "@rpath/" if path == executable else "@loader_path/"
            args += ["-change", original, prefix + target.name]
        if path == executable:
            args += ["-add_rpath", "@executable_path/../Resources/container-runtime", "-add_rpath", "@executable_path/../../resources/container-runtime"]
        subprocess.run([*args, str(dest)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for path in [*runtime.iterdir(), output / "kg-mime-unit"]:
        subprocess.run(["codesign", "--force", "--sign", "-", str(path)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    licenses = output / "container-licenses"
    licenses.mkdir(exist_ok=True)
    for formula in ["gmime", "gpgme", "gettext", "libidn2", "libunistring", "pcre2", "libgpg-error", "libassuan"]:
        prefix = Path(subprocess.check_output(["brew", "--prefix", formula], text=True).strip())
        for source in prefix.glob("COPYING*"):
            if source.is_file():
                shutil.copy2(source, licenses / f"{formula}-{source.name}")
    (licenses / "NOTICE.txt").write_text("GMime 3.2.15 and its pinned Homebrew dependency closure include LGPL, GPL and BSD licensed components. GLib is LGPL-2.1-or-later; its license text is included as gmime-COPYING. Source and licenses: https://github.com/jstedfast/gmime and https://gitlab.gnome.org/GNOME/glib. See the bundled COPYING files and gmime-runtime-lock.json for library identities.\n")
    shutil.copy2(here / "gmime-runtime-lock.json", licenses / "gmime-runtime-lock.json")
print(output)
