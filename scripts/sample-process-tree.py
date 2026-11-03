#!/usr/bin/env python3
"""Sample a macOS process tree; report kernel lifetime peaks for observed PIDs.

Run during a documented workload: python3 scripts/sample-process-tree.py PID
Stop with Ctrl-C after the workload. Children that start and exit between samples
may be missed. The sampled sum can double-count shared memory and is not a
machine-wide peak. Do not record private command arguments.
"""

import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

if len(sys.argv) != 2 or not sys.argv[1].isdigit() or int(sys.argv[1]) <= 0:
    sys.exit("usage: sample-process-tree.py PID")

root = int(sys.argv[1])
source = Path(__file__).with_name("process-footprint.c")
observed = {}
max_sampled_sum = 0
samples = 0
started = time.monotonic()
root_start = None

with tempfile.TemporaryDirectory(prefix="kg-footprint-") as temp:
    probe = str(Path(temp) / "process-footprint")
    subprocess.run(["clang", "-Wall", "-Wextra", "-Werror", str(source), "-o", probe], check=True)
    try:
        while True:
            result = subprocess.run(["ps", "-axo", "pid=,ppid=,state=,comm="], capture_output=True, text=True, check=True)
            processes = {}
            for line in result.stdout.splitlines():
                fields = line.strip().split(None, 3)
                if len(fields) == 4 and fields[0].isdigit() and fields[1].isdigit() and not fields[2].startswith("Z"):
                    processes[int(fields[0])] = (int(fields[1]), Path(fields[3]).name)
            if root not in processes:
                break
            tree = {root}
            while True:
                expanded = tree | {pid for pid, (parent, _) in processes.items() if parent in tree}
                if expanded == tree:
                    break
                tree = expanded
            total = 0
            for pid in tree:
                sample = subprocess.run([probe, str(pid)], capture_output=True, text=True)
                if sample.returncode:
                    continue
                data = json.loads(sample.stdout)
                if pid == root:
                    if root_start is None:
                        root_start = data["start_abstime"]
                    elif root_start != data["start_abstime"]:
                        tree = set()
                        break
                total += data["physical_bytes"]
                identity = f"{pid}:{data['start_abstime']}"
                previous = observed.get(identity, {"name": processes[pid][1], "peak_bytes": 0})
                previous["peak_bytes"] = max(previous["peak_bytes"], data["lifetime_peak_physical_bytes"])
                observed[identity] = previous
            if not tree:
                break
            max_sampled_sum = max(max_sampled_sum, total)
            samples += 1
            time.sleep(0.25)
    except KeyboardInterrupt:
        pass

print(json.dumps({"root_pid": root, "elapsed_seconds": round(time.monotonic() - started, 2),
                  "samples": samples, "max_sampled_tree_sum_bytes": max_sampled_sum,
                  "observed_processes": observed}, indent=2))
