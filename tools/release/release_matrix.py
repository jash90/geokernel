"""List the native GitHub-hosted runner for each enabled release variant."""

import json
import sys


def release_matrix(with_giac):
    platforms = [
        ("linux", "x86_64", "ubuntu-latest"),
        ("windows", "x86_64", "windows-latest"),
        ("macos", "arm64", "macos-latest"),
    ]
    include = [
        dict(platform=platform, arch=arch, runner=runner, variant="eupl")
        for platform, arch, runner in platforms
    ]
    if with_giac:
        include.extend(
            dict(platform=platform, arch=arch, runner=runner, variant="gpl")
            for platform, arch, runner in platforms[:2] + [("macos", "x86_64", "macos-15-intel")]
        )
    return {"include": include}


if __name__ == "__main__":
    print(json.dumps(release_matrix(sys.argv[1] == "true")))
