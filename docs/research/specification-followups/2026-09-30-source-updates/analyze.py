"""Offline audit and Markdown fixture diffs for one retained run; no API calls."""
import difflib
import hashlib
import json
from pathlib import Path
import sys

run = Path(sys.argv[1]).resolve()
summary = json.loads((run / "summary.json").read_text())
frozen = json.loads((run / "frozen-inputs.json").read_text())
cases = {c["id"]: c for c in frozen["cases"]}
evaluations = summary["evaluations"]
output = run / "offline-audit"
output.mkdir(exist_ok=False)


def render(groups):
    lines = ["# Controlled observation fixture", "", "Synthetic research data. This is a record-rendering example, not an application export.", ""]
    for label, records in groups.items():
        if not records:
            continue
        lines += ["## " + label, "", "| Field | Value | Supporting source | Exact excerpt |", "| --- | --- | --- | --- |"]
        for field, record in records.items():
            cells = [field, str(record["value"]), record["source_id"], record["quote"]]
            lines.append("| " + " | ".join(v.replace("|", "\\|") for v in cells) + " |")
        lines.append("")
    return "\n".join(lines)


examples = []
for case_id in ["replacement", "correction", "separate_event"]:
    case = cases[case_id]
    evaluation = next(e for e in evaluations if e["trial"] == case_id + "-r1")
    before = render({"Existing observation": case["current_source"]["candidates"]})
    proposal = evaluation["proposal"]
    after = render({"Existing observation": proposal["current_observation"], "Separate observation": proposal["separate_observation"]})
    prefix = output / case_id
    prefix.with_suffix(".before.md").write_text(before)
    prefix.with_suffix(".after.md").write_text(after)
    diff = "".join(difflib.unified_diff(before.splitlines(keepends=True), after.splitlines(keepends=True), fromfile=case_id + ".before.md", tofile=case_id + ".after.md"))
    prefix.with_suffix(".diff").write_text(diff)
    examples.append({"case": case_id, "source_trial": evaluation["trial"], "proposal_matches_expected": evaluation["proposal_matches_expected"],
                     "before_sha256": hashlib.sha256(before.encode()).hexdigest(), "after_sha256": hashlib.sha256(after.encode()).hexdigest(),
                     "boundary": "Offline deterministic rendering of the actually selected Jev field operations. Does not demonstrate parsing, external-editor writes or Obsidian."})

complete = [e for e in evaluations if not e["expected"]["needs_more_content"]]
result = {
    "source_summary_sha256": hashlib.sha256((run / "summary.json").read_bytes()).hexdigest(),
    "main_trials": sum(e["case"] in ["replacement", "correction", "separate_event"] for e in evaluations),
    "trial_count_including_boundaries_and_recovery": len(evaluations),
    "relationship_matches": sum(e["checks"]["relationship"] for e in evaluations),
    "field_operations": sum(len(e["checks"]["fields"]) for e in evaluations),
    "field_operation_matches": sum(sum(e["checks"]["fields"].values()) for e in evaluations),
    "proposals_matching_expected": sum(e["proposal_matches_expected"] for e in evaluations),
    "complete_inputs": len(complete),
    "unnecessary_more_content_flags_at_prefixed_half_cutoff": sum(e["answers"]["needs_more_content"]["noul"] >= .5 for e in complete),
    "correct_proposals_rejected_by_broad_verifier": [e["trial"] for e in evaluations if e["proposal_matches_expected"] and e["verifier_answers"]["verdict"]["choice"] != "consistent"],
    "seeded_errors": len(summary["negative_controls"]),
    "seeded_errors_detected": sum(e["answers"]["verdict"]["choice"] == "incorrect" for e in summary["negative_controls"]),
    "seeded_errors_missed": [e["case"] for e in summary["negative_controls"] if e["answers"]["verdict"]["choice"] != "incorrect"],
    "examples": examples,
    "interpretation": [
        "Q18 omission semantics remain a candidate policy; model agreement does not accept policy for the user.",
        "Complete source content and pre-extracted candidate fields are authored fixture inputs, not real extraction results.",
        "Date ordering was computed in code and supplied to Jev; source-age extraction was not tested.",
        "Independent questions can disagree. The whole-record verifier missed an erroneous observer deletion and rejected a correct summary update.",
        "The broad more-content predicate is over-triggered under its frozen 0.5 diagnostic cutoff. This is not a calibrated production threshold.",
        "Actual extraction-completeness metadata and structural output checks should be enforced in code; semantic questions alone are not integrity guarantees.",
        "Incomplete-extraction recovery revealed frozen additional source text after a model signal, then made a new request. It was not a real PDF fetch or extraction.",
        "Repeated small fixtures provide no general accuracy estimate or zero-maintenance product guarantee."
    ]
}
(output / "analysis.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({k:v for k,v in result.items() if k not in ["examples", "interpretation"]}, indent=2))
