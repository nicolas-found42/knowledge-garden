"""Executable assertion at the public Application process seam, without compiling.

Pass a provenance-verified application_driver and evidence destination. This uses
the real LocalExtractor; no semantic provider or PDF implementation is mocked.
"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile

driver = Path(sys.argv[1]).resolve()
evidence = Path(sys.argv[2]).resolve()
evidence.mkdir(parents=True, exist_ok=True)
fixture = Path(__file__).resolve().parent / "source" / "text-report.pdf"
manifest = json.loads((fixture.parent.parent / "manifest.json").read_text())
expected = next(row for row in manifest["source_files"] if row["file"] == fixture.name)
assert hashlib.sha256(fixture.read_bytes()).hexdigest() == expected["sha256"]

with tempfile.TemporaryDirectory(prefix="kg-pdf-public-") as temporary:
    collection = Path(temporary) / "collection"

    def call(operation, *args):
        result = subprocess.run([str(driver), str(collection), operation, *args],
                                capture_output=True, text=True, check=True)
        (evidence / f"{operation}.json").write_text(result.stdout)
        (evidence / f"{operation}.stderr").write_text(result.stderr)
        return json.loads(result.stdout)

    imported = call("import", str(fixture), "picker")
    source_id = imported["info"]["source_id"]
    page = call("open", source_id)
    retained = Path(call("original", source_id))
    results = call("search", "V17")
    assert retained.read_bytes() == fixture.read_bytes(), "original bytes changed"
    assert page["info"]["format"] == "pdf"
    assert page["info"]["extraction"] in ("structured_text", "partial_text"), (
        "PDF must produce a readable located projection; observed "
        + page["info"]["extraction"] + ": " + page["info"]["extraction_detail"])
    for text in ["PDF page 1", "PDF page 2",
                 "V17 recorded 15 visits at Riverside on May 17, 2024.",
                 "V17 lasted 10 minutes, excluding setup time.",
                 "The duration is provisional pending calibration."]:
        assert text in page["body"], f"missing independently labeled evidence: {text}"
    assert page["info"]["extraction_coverage"], "page coverage absent"
    assert any(row["page_id"] == source_id for row in results["pages"]), (
        "imported PDF evidence must be searchable")
print("PASS: PDF original, source pages, qualifiers, coverage and search")
