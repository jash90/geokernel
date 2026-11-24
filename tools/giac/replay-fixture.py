#!/usr/bin/env python3
"""Convert the reference harness's Giac JSON log into a replay fixture."""
import json
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


def apply_overrides(base_path, override_path):
    with open(override_path, encoding="utf-8") as overrides:
        expected = {}
        for line in overrides:
            case, text, result = json.loads(line)
            key = (case, text)
            if key in expected:
                raise ValueError(f"duplicate replay override for {case}")
            expected[key] = result
    used = set()
    with open(base_path, encoding="utf-8") as base:
        for line in base:
            case, text, result = json.loads(line)
            key = (case, text)
            if key in expected:
                used.add(key)
                result = expected[key]
            yield (case, text, result)
    if unused := expected.keys() - used:
        raise ValueError(f"unused replay overrides: {sorted(case for case, _ in unused)}")


if __name__ == "__main__":
    args = sys.argv[1:]
    if len(args) == 1:
        records = from_harness(args[0])
    elif len(args) == 3 and args[0] == "--overlay":
        records = apply_overrides(*args[1:])
    else:
        sys.exit("usage: replay-fixture.py <harness log> | --overlay <base fixture> <overrides>")
    sys.stdout.reconfigure(encoding="utf-8", newline="\n")
    for record in records:
        print(json.dumps(record, ensure_ascii=False))
