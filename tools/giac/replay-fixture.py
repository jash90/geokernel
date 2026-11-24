#!/usr/bin/env python3
"""Convert the reference harness's Giac JSON log into a replay fixture."""
import json
import sys


def main():
    pending = None
    with open(sys.argv[1], encoding="utf-8") as log:
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
                print(json.dumps((*pending, event["text"]), ensure_ascii=False))
                pending = None
            elif kind == "giac evalRaw canceled":
                pending = None
            else:
                raise ValueError(f"unknown Giac log kind {event['kind']}")


if __name__ == "__main__":
    main()
