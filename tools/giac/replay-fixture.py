#!/usr/bin/env python3
"""Convert the reference harness's Giac JSON log into a replay fixture."""
import argparse
import hashlib
from itertools import islice
import json
from pathlib import Path
import sys


def from_harness(log_path):
    pending = None
    with open(log_path, encoding="utf-8") as log:
        for line in log:
            event = json.loads(line)
            if event["event"] != "raw":
                continue
            kind = event["kind"]
            if kind == "giac evalRaw input:":
                text = event["text"]
                if not text.startswith("evalfa(") or not text.endswith(")"):
                    raise ValueError(f"invalid Giac input for {event['case']}")
                pending = (event["case"], text[len("evalfa("):-1])
            elif kind == "giac evalRaw output:":
                if pending is None or pending[0] != event["case"]:
                    raise ValueError(f"unpaired Giac output for {event['case']}")
                yield (*pending, event["text"])
                pending = None
            elif kind == "giac evalRaw canceled":
                pending = None
            else:
                raise ValueError(f"unknown Giac log kind {event['kind']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--harness-revision", required=True)
    parser.add_argument("--reference-revision", required=True)
    parser.add_argument("--date", required=True)
    parser.add_argument("--command", required=True, help="recording command with portable paths")
    args = parser.parse_args()

    exchange_count = 1_966
    records = list(islice(from_harness(args.log), exchange_count + 1))
    if len(records) != exchange_count:
        parser.error(f"expected exactly {exchange_count} complete exchanges")
    sha256 = hashlib.sha256()
    with args.library.open("rb") as library:
        for chunk in iter(lambda: library.read(1024 * 1024), b""):
            sha256.update(chunk)
    header = {
        "type": "provenance",
        "platform": args.platform,
        "library_file": args.library.name,
        "library_sha256": sha256.hexdigest(),
        "harness_revision": args.harness_revision,
        "reference_revision": args.reference_revision,
        "date": args.date,
        "command": args.command,
    }
    sys.stdout.reconfigure(encoding="utf-8", newline="\n")
    print(json.dumps(header, ensure_ascii=False))
    for record in records:
        print(json.dumps(record, ensure_ascii=False))


if __name__ == "__main__":
    main()
