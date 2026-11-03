# Pinned MIME build distribution

This archive contains the Apple Silicon GMime 3.2.15 dynamic runtime, its exact dependency closure, matching compilation headers, and component license texts. It is about 3.2 MiB compressed. The build uses the platform C compiler and these files without querying or linking an installed Homebrew package.

`manifest.json` authenticates the archive and every extracted file, identifies each component's source URL and source-archive hash, and binds the package to `container-helper/gmime-runtime-lock.json`. The latter records the reviewed hashes of the original installed libraries, before relocation and ad hoc signing. The distribution manifest records the relocated files actually bundled. GLib's license is included explicitly.

`provision_gmime.py` verifies the archive, bounds extraction, rejects links, traversal, unexpected or missing files, checks architecture and the closed dynamic dependency set, then compiles the repository's `gmime_unit.c`. It copies the unchanged runtime and licenses into the generated resources and signs the helper. The application opens MIME inputs under its existing sandbox.

To regenerate on a machine with the exact reviewed source closure:

```sh
python3 src-tauri/container-helper/refresh_gmime_distribution.py
python3 -m unittest discover -s src-tauri/container-helper -p 'test_*.py'
```

A dependency upgrade requires deliberate review of the source closure lock and the distribution's origins, file hashes, and license changes. The refresh command rejects an installed closure that differs from that lock. Build generation never refreshes the pins.
