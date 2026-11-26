"""Collect redistribution notices from the crates in the release build."""

import json
from pathlib import Path
import shutil
import subprocess

LICENSES = Path(__file__).with_name("licenses")
# These upstream projects publish some crates without their repository licence files.
FALLBACKS = {
    "github.com/emilk/egui": ("egui-*",),
    "github.com/accesskit/accesskit": ("accesskit-*",),
    "github.com/madsmtm/objc2": ("objc2-*",),
    "github.com/gfx-rs/rspirv": ("rspirv-*", "spirv-headers-*"),
    "github.com/aclysma/profiling": ("profiling-*",),
    "github.com/doumanash/clipboard-win": ("clipboard-win-*",),
    "github.com/brendanzab/gl-rs": ("gl-rs-*",),
    "github.com/ssheldon/rust-dispatch": ("rust-dispatch-*",),
}


def copy_crate_notices(messages, destination):
    import tomllib

    manifests = set()
    with messages.open(encoding="utf-8") as build:
        for line in build:
            event = json.loads(line)
            if event.get("reason") != "compiler-artifact":
                continue
            if event["package_id"].startswith("registry+"):
                manifests.add(Path(event["manifest_path"]))
    if not manifests:
        raise ValueError("no registry dependencies in the desktop release build")

    entries = []
    for manifest in sorted(manifests, key=lambda path: path.parent.name):
        source = manifest.parent
        package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]
        name = f'{package["name"]}-{package["version"]}'
        repo = package.get("repository", "")
        key = repo.lower().split("://", 1)[-1].split("/tree/", 1)[0]
        key = key.rstrip("/").removesuffix(".git")
        dest = destination / name
        files = {
            path for path in source.rglob("*")
            if path.is_file() and path.name.lower().startswith(
                ("license", "licence", "copying", "copyright", "notice", "unlicense", "third-party")
            )
            and path.suffix.lower() in ("", ".txt", ".md", ".html", ".mit", ".apache", ".bsd", ".unicode")
        }
        if "license-file" in package:
            files.add(source / package["license-file"])
        for path in sorted(files):
            target = dest / path.relative_to(source)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
        for pattern in FALLBACKS.get(key, ()):
            for path in (LICENSES / "crate-fallbacks").glob(pattern):
                dest.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, dest / path.name)
        if not dest.exists():
            raise ValueError(f"{name} has no attribution or licence files ({repo})")
        entries.append(f'{name} | {package.get("license") or package["license-file"]} | {repo}')
    (destination / "COMPONENTS.txt").write_text("\n".join(entries) + "\n", encoding="utf-8")


def copy_rust_notices(destination):
    sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
    docs = Path(sysroot) / "share/doc/rust"
    shutil.copytree(docs / "licenses", destination)
    shutil.copyfile(docs / "COPYRIGHT-library.html", destination / "COPYRIGHT-library.html")
