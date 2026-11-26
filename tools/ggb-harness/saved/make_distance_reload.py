#!/usr/bin/env python3
"""Record the reader's values for line-distance commands whose saved order matters."""

import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
FILES = {
    "conic/conic-diameter-009": ["d"],
    "lines/lines-bisector-015": ["d", "e"],
    "transform/transform-mirrorline-048": ["e"],
    "transform/transform-rotate-037": ["e"],
}

records = {}
for name, labels in FILES.items():
    xml = f"golden/ggb/{name}.xml"
    run = subprocess.run(
        [ROOT / "tools/ggb-harness/linux-x86_64.sh", "reload", xml, *labels],
        cwd=ROOT, check=True, text=True, stdout=subprocess.PIPE,
    )
    filename = Path(xml).name
    values = dict(line.split(" = ", 1) for line in run.stdout.splitlines()
                  if line.startswith(f"{filename} "))
    records[Path(name).name] = {
        label: float(values[f"{filename} {label}"]) for label in labels
    }

output = ROOT / "golden/ggb/distance-reload.snapshot"
output.write_text(json.dumps(records, indent=2) + "\n")
