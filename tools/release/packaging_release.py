"""Stage release payloads and package and smoke-test them on the native runner."""

import argparse
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

from release_icon import write_icon

REPO = Path(__file__).resolve().parents[2]
GIAC = REPO / "tools/giac"
LIBRARIES = {"macos": "libjavagiac.jnilib", "windows": "javagiac64.dll", "linux": "libjavagiac64.so"}
SOURCE_FILES = ("giac-70507-source.archive", "giac-dependency-source.tar.gz", "GPL-3.0.txt")


@dataclass(frozen=True)
class ReleaseConfig:
    version: str
    product: str
    identifier: str
    linux_formats: str

    def __post_init__(self):
        if not re.fullmatch(r"(0|[1-9][0-9]{0,3})(\.(0|[1-9][0-9]?)){1,2}", self.version):
            raise ValueError("version must be a numeric major.minor or major.minor.patch")
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 _.-]{0,47}", self.product):
            raise ValueError("product name must be 1–48 ASCII letters, digits, spaces, underscores, dots or hyphens")
        if self.product.endswith((" ", ".")) or "geogebra" in self.product.lower():
            raise ValueError("product name must be distinct and safe as a directory name")
        if not re.fullmatch(r"[a-z][a-z0-9-]*(\.[a-z][a-z0-9-]*){2,}", self.identifier):
            raise ValueError("application identifier must be a reverse-DNS name in lowercase")
        if self.linux_formats not in ("both", "appimage", "deb"):
            raise ValueError("Linux formats must be both, appimage, or deb")


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_checksums(directory):
    sums = directory / "SHA256SUMS"
    files = sorted(path for path in directory.iterdir() if path.is_file() and path != sums)
    if not files:
        raise ValueError("no assets available for checksums")
    sums.write_bytes("".join(f"{sha256(path)}  {path.name}\n" for path in files).encode("ascii"))


def download(url, digest, dest):
    if not url.startswith("https://") or not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise ValueError("download requires an HTTPS URL and a lowercase SHA-256")
    run("curl", "--fail", "--silent", "--show-error", "--location", "--retry", "2",
        "--max-time", "360", "--proto", "=https", "--proto-redir", "=https",
        "--output", dest, "--", url)
    if sha256(dest) != digest:
        dest.unlink()
        raise ValueError(f"SHA-256 mismatch for {dest.name}")
    return dest


def package_id(variant):
    return "geokernel-giac" if variant == "gpl" else "geokernel"


def display_name(config, variant):
    return config.product + (" with Giac" if variant == "gpl" else "")


def stage_payload(config, platform, arch, variant, binary, dest, sources=None, library=None):
    name = package_id(variant)
    if variant == "gpl":
        if arch != "x86_64" or sources is None or library is None or library.name != LIBRARIES[platform]:
            raise ValueError("GPL payload requires the supported pinned Giac library and complete source")
        for filename in SOURCE_FILES:
            if not (sources / filename).is_file():
                raise FileNotFoundError(sources / filename)
    libdir = dest
    docs = dest
    if platform == "macos":
        contents = dest / "Contents"
        bindir = contents / "MacOS"
        libdir = contents / "Frameworks"
        docs = contents / "Resources"
        docs.mkdir(parents=True)
        write_icon(docs / "geokernel.icns")
        mac_version = config.version if config.version.count(".") == 2 else f"{config.version}.0"
        with (contents / "Info.plist").open("wb") as plist:
            plistlib.dump({
                "CFBundleExecutable": binary.name,
                "CFBundleIdentifier": config.identifier + (".giac" if variant == "gpl" else ""),
                "CFBundleName": display_name(config, variant),
                "CFBundleDisplayName": display_name(config, variant),
                "CFBundlePackageType": "APPL",
                "CFBundleShortVersionString": mac_version,
                "CFBundleVersion": config.version,
                "CFBundleIconFile": "geokernel.icns",
                "NSHighResolutionCapable": True,
            }, plist)
    elif platform == "windows":
        bindir = dest
        docs.mkdir(parents=True)
        write_icon(docs / "geokernel.ico")
    else:
        bindir = libdir = dest / "usr/lib" / name
        docs = dest / "usr/share/doc" / name
        docs.mkdir(parents=True)

    bindir.mkdir(parents=True, exist_ok=True)
    app = bindir / binary.name
    shutil.copy2(binary, app)
    app.chmod(0o755)
    for filename in ("NOTICE", "LICENSE"):
        shutil.copyfile(REPO / filename, docs / filename)
    head = run("git", "rev-parse", "HEAD", cwd=REPO, capture_output=True, text=True).stdout.strip()
    (docs / "REVISION").write_text(f"geokernel {head}\nPlatform {platform}-{arch}\n", encoding="ascii")
    if variant == "gpl":
        libdir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(library, libdir / library.name)
        for filename in SOURCE_FILES:
            shutil.copyfile(sources / filename, docs / filename)
        with (docs / "NOTICE").open("a", encoding="utf-8") as notice:
            notice.write("\nGiac 70507 corresponding source\n")
            for filename in SOURCE_FILES[:2]:
                notice.write(f"SHA-256: {sha256(sources / filename)}  {filename}\n")
        with (docs / "REVISION").open("a", encoding="ascii") as revision:
            revision.write(f"Giac javagiac 70507 ({library.name})\n")
        shutil.copyfile(GIAC / "source-offer.txt", docs / "SOURCE-OFFER.txt")
        run("git", "archive", "--format=tar.gz", "--prefix=geokernel-source/",
            "-o", docs / "geokernel-source.tar.gz", head, cwd=REPO)
    return app


def smoke(config, variant, *command):
    env = os.environ.copy()
    env.pop("GEO_GIAC_LIBRARY", None)
    expected = f"geokernel {config.version} | "
    expected += "GPL-3.0-or-later with Giac | Giac loaded" if variant == "gpl" else "EUPL-1.2 | Giac disabled"
    result = run(*command, "--version", env=env, capture_output=True, text=True, timeout=60)
    if result.stdout.strip() != expected:
        raise RuntimeError(f"Unexpected version output: {result.stdout!r} {result.stderr!r}")
    print(f"{command[0]}: {expected}", flush=True)


def source_gate(directory):
    """Verify the pinned publisher tree and a separately reviewed dependency bundle."""
    source_path = run("bash", "tools/giac/fetch-source.sh", cwd=REPO, capture_output=True, text=True).stdout.strip()
    if os.name == "nt":
        source_path = run("bash", "-c", 'cygpath -am "$1"', "bash", source_path,
                          capture_output=True, text=True).stdout.strip()
    deps = download(
        os.getenv("GIAC_DEPENDENCY_SOURCE_URL", ""),
        os.getenv("GIAC_DEPENDENCY_SOURCE_SHA256", ""),
        directory / SOURCE_FILES[1],
    )
    # The owner verifies correspondence to the published binaries before setting
    # the repository pin. Check the archive layout without extracting its contents.
    with tarfile.open(deps, "r:gz") as archive:
        entries = [entry.name.removeprefix("./") for entry in archive if entry.isfile()]
    for platform in ("linux-x86_64", "macos-x86_64", "windows-x86_64"):
        if (f"{platform}/BUILD.md" not in entries
                or not any(name.startswith(f"{platform}/gmp/") for name in entries)
                or not any(name.startswith(f"{platform}/mpfr/") for name in entries)):
            raise ValueError(f"dependency source is missing source files or build instructions for {platform}")
    shutil.copyfile(source_path, directory / SOURCE_FILES[0])


def fetch_gpl(directory):
    source_gate(directory)
    download("https://www.gnu.org/licenses/gpl-3.0.txt",
             "3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986",
             directory / SOURCE_FILES[2])
    return Path(run("bash", "tools/giac/fetch.sh", cwd=REPO, capture_output=True, text=True).stdout.strip())


def package_linux(config, variant, appdir, out, stem, work):
    name = package_id(variant)
    desktop = (
        "[Desktop Entry]\nType=Application\n"
        f"Name={display_name(config, variant)}\nExec={name}\n"
        f"Icon={name}\nCategories=Education;Math;\nTerminal=false\n"
    )
    for file in (f"usr/share/applications/{name}.desktop", f"usr/bin/{name}",
                 f"usr/share/icons/hicolor/256x256/apps/{name}.png"):
        (appdir / file).parent.mkdir(parents=True, exist_ok=True)
    desktop_file = appdir / f"usr/share/applications/{name}.desktop"
    desktop_file.write_text(desktop, encoding="ascii")
    icon = appdir / f"usr/share/icons/hicolor/256x256/apps/{name}.png"
    write_icon(icon)
    (appdir / f"usr/bin/{name}").symlink_to(f"../lib/{name}/geokernel-app")

    if config.linux_formats in ("appimage", "both"):
        tool = download(
            "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage",
            "ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0",
            work / "appimagetool.AppImage",
        )
        runtime = download(
            "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64",
            "2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d",
            work / "runtime-x86_64",
        )
        tool.chmod(0o755)
        shutil.copyfile(desktop_file, appdir / desktop_file.name)
        shutil.copyfile(icon, appdir / icon.name)
        (appdir / ".DirIcon").symlink_to(icon.name)
        launcher = appdir / "AppRun"
        launcher.write_text(f'#!/bin/sh\nexec "$APPDIR/usr/bin/{name}" "$@"\n', encoding="ascii")
        launcher.chmod(0o755)
        image = out / f"{stem}.AppImage"
        run(tool, "--appimage-extract-and-run", "--runtime-file", runtime, appdir, image,
            env=os.environ | {"ARCH": "x86_64"}, cwd=work)
        smoke(config, variant, image, "--appimage-extract-and-run")
        for path in (appdir / desktop_file.name, appdir / icon.name, appdir / ".DirIcon", launcher):
            path.unlink()

    if config.linux_formats in ("deb", "both"):
        metadata = appdir / "DEBIAN"
        metadata.mkdir()
        (metadata / "control").write_text(
            f"Package: {name}\nVersion: {config.version}\nSection: education\n"
            "Priority: optional\nArchitecture: amd64\n"
            "Maintainer: geokernel project <https://github.com/jash90/geokernel>\n"
            "Depends: libc6 (>= 2.39), libgcc-s1, libxkbcommon0, libx11-6, libxcb1\n"
            "Recommends: mesa-vulkan-drivers\n"
            f"Description: {display_name(config, variant)} desktop geometry application\n",
            encoding="ascii",
        )
        shutil.copyfile(appdir / f"usr/share/doc/{name}/LICENSE",
                        appdir / f"usr/share/doc/{name}/copyright")
        deb = out / f"{stem}.deb"
        run("dpkg-deb", "--root-owner-group", "--build", appdir, deb)
        run("sudo", "dpkg", "-i", deb)
        smoke(config, variant, f"/usr/bin/{name}")


def package_macos(config, variant, app, out, stem, work, sign):
    import base64
    import secrets

    bundle_id = config.identifier + (".giac" if variant == "gpl" else "")
    keychain = work / "release.keychain-db"
    has_keychain = False
    try:
        if sign:
            p12 = work / "apple.p12"
            key = work / "notary.p8"
            p12.write_bytes(base64.b64decode(os.environ["APPLE_CERTIFICATES_P12"], validate=True))
            key.write_bytes(base64.b64decode(os.environ["APPLE_NOTARY_KEY"], validate=True))
            key.chmod(0o600)
            password = secrets.token_hex(24)
            run("security", "create-keychain", "-p", password, keychain)
            has_keychain = True
            run("security", "unlock-keychain", "-p", password, keychain)
            run("security", "import", p12, "-k", keychain, "-f", "pkcs12",
                "-P", os.environ["APPLE_CERTIFICATES_PASSWORD"], "-T", "/usr/bin/codesign",
                "-T", "/usr/bin/productsign")
            run("security", "set-key-partition-list", "-S", "apple-tool:,apple:", "-s",
                "-k", password, keychain)
            entitlements = []
            if variant == "gpl":
                # The Giac loader verifies the exact publisher bytes. Sign the
                # outer bundle and allow its executable to load that library.
                plist = work / "giac-entitlements.plist"
                with plist.open("wb") as handle:
                    plistlib.dump({"com.apple.security.cs.disable-library-validation": True}, handle)
                entitlements = ["--entitlements", plist]
            run("codesign", "--force", "--options", "runtime", "--timestamp", "--keychain",
                keychain, "--sign", os.environ["APPLE_APPLICATION_IDENTITY"], *entitlements, app)
            run("codesign", "--verify", "--strict", app)

        def notarize(asset):
            run("xcrun", "notarytool", "submit", asset, "--wait", "--timeout", "20m", "--key", key,
                "--key-id", os.environ["APPLE_NOTARY_KEY_ID"], "--issuer", os.environ["APPLE_NOTARY_ISSUER"])

        if sign:
            app_for_notary = work / "notarization.zip"
            run("ditto", "-c", "-k", "--keepParent", app, app_for_notary)
            notarize(app_for_notary)
            run("xcrun", "stapler", "staple", app)

        archive = out / f"{stem}.app.zip"
        run("ditto", "-c", "-k", "--keepParent", app, archive)
        extracted = work / "extracted"
        run("ditto", "-x", "-k", archive, extracted)
        smoke(config, variant, extracted / app.name / "Contents/MacOS/geokernel-app")

        payload = work / "dmg-payload"
        payload.mkdir()
        run("ditto", app, payload / app.name)
        (payload / "Applications").symlink_to("/Applications")
        dmg = out / f"{stem}.dmg"
        run("hdiutil", "create", "-quiet", "-format", "UDZO", "-fs", "HFS+", "-srcfolder",
            payload, "-volname", display_name(config, variant), dmg)
        if sign:
            run("codesign", "--timestamp", "--keychain", keychain, "--sign",
                os.environ["APPLE_APPLICATION_IDENTITY"], dmg)
            notarize(dmg)
            run("xcrun", "stapler", "staple", dmg)
        mount = work / "mounted"
        run("hdiutil", "attach", "-quiet", "-nobrowse", "-readonly", "-mountpoint", mount, dmg)
        try:
            smoke(config, variant, mount / app.name / "Contents/MacOS/geokernel-app")
        finally:
            run("hdiutil", "detach", mount)

        component = work / "component.pkg"
        run("pkgbuild", "--component", app, "--identifier", bundle_id, "--version",
            config.version, "--install-location", "/Applications", component)
        pkg = out / f"{stem}.pkg"
        if sign:
            unsigned = work / "distribution.pkg"
            run("productbuild", "--package", component, unsigned)
            run("productsign", "--keychain", keychain, "--sign", os.environ["APPLE_INSTALLER_IDENTITY"],
                unsigned, pkg)
            notarize(pkg)
            run("xcrun", "stapler", "staple", pkg)
        else:
            run("productbuild", "--package", component, pkg)
        installed = Path("/Applications") / app.name
        if installed.exists():
            raise ValueError(f"refusing to smoke test over an existing application: {installed}")
        try:
            run("sudo", "installer", "-pkg", pkg, "-target", "/")
            smoke(config, variant, installed / "Contents/MacOS/geokernel-app")
        finally:
            if installed.exists():
                run("sudo", "rm", "-r", installed)
    finally:
        if has_keychain:
            run("security", "delete-keychain", keychain)


def package_windows(config, variant, stage, out, stem, work, sign):
    import base64

    tool_installer = download(
        "https://github.com/jrsoftware/issrc/releases/download/is-6_5_4/innosetup-6.5.4.exe",
        "fa73bf47a4da250d185d07561c2bfda387e5e20db77e4570004cf6a133cc10b1",
        work / "inno-setup.exe",
    )
    inno = work / "inno"
    silent = ("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART")
    run(tool_installer, *silent, "/SP-", "/CURRENTUSER", "/PORTABLE=1", f"/DIR={inno}", timeout=180)
    if sign:
        pfx = work / "windows.pfx"
        pfx.write_bytes(base64.b64decode(os.environ["WINDOWS_SIGN_PFX"], validate=True))
        sdk = Path(os.environ["ProgramFiles(x86)"]) / "Windows Kits/10/bin"
        signtool = sorted(sdk.glob("*/x64/signtool.exe"))[-1]

    def sign_file(path):
        run(signtool, "sign", "/fd", "SHA256", "/f", pfx, "/p", os.environ["WINDOWS_SIGN_PASSWORD"],
            "/tr", "http://timestamp.digicert.com", "/td", "SHA256", path)
        run(signtool, "verify", "/pa", path)

    if sign:
        sign_file(stage / "geokernel-app.exe")
    license_file = "GPL-3.0.txt" if variant == "gpl" else "LICENSE"
    run(inno / "ISCC.exe", f"/DAppName={display_name(config, variant)}",
        f"/DAppVersion={config.version}", f"/DAppId={config.identifier}.{variant}",
        f"/DInstallName={package_id(variant)}", f"/DPayload={stage}", f"/DLicenseFile={license_file}",
        f"/O{out}", f"/F{stem}-setup", REPO / "tools/release/windows.iss")
    installer = out / f"{stem}-setup.exe"
    if sign:
        sign_file(installer)
    installed = work / "installed"
    run(installer, *silent, "/SP-", f"/DIR={installed}", timeout=180)
    try:
        smoke(config, variant, installed / "geokernel-app.exe")
    finally:
        run(installed / "unins000.exe", *silent, timeout=180)


def package(args, config):
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="geokernel-release-") as directory:
        work = Path(directory)
        sources = library = None
        if args.variant == "gpl":
            sources = work / "sources"
            sources.mkdir()
            library = fetch_gpl(sources)
        dest = work / (f"{display_name(config, args.variant)}.app" if args.platform == "macos" else "payload")
        stage_payload(config, args.platform, args.arch, args.variant, args.binary, dest, sources, library)
        stem = f"{package_id(args.variant)}-{config.version}-{args.platform}-{args.arch}"
        if args.platform == "macos":
            package_macos(config, args.variant, dest, out, stem, work, args.sign)
        elif args.platform == "windows":
            package_windows(config, args.variant, dest, out, stem, work, args.sign)
        else:
            package_linux(config, args.variant, dest, out, stem, work)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("source-gate")
    commands.add_parser("validate")
    commands.add_parser("checksums").add_argument("out", type=Path)
    build = commands.add_parser("package")
    build.add_argument("platform", choices=("linux", "macos", "windows"))
    build.add_argument("arch", choices=("x86_64", "arm64"))
    build.add_argument("variant", choices=("eupl", "gpl"))
    build.add_argument("binary", type=Path)
    build.add_argument("out", type=Path)
    build.add_argument("--sign", action="store_true")
    args = parser.parse_args()
    try:
        if args.command == "checksums":
            write_checksums(args.out)
        elif args.command == "source-gate":
            with tempfile.TemporaryDirectory() as directory:
                source_gate(Path(directory))
        else:
            config = ReleaseConfig(os.environ["RELEASE_VERSION"], os.environ["PRODUCT_NAME"],
                                   os.environ["APP_IDENTIFIER"], os.environ["LINUX_FORMATS"])
            if args.command == "package":
                package(args, config)
    except subprocess.TimeoutExpired as error:
        parser.exit(1, f"{Path(error.cmd[0]).name} timed out after {error.timeout} seconds\n")
    except subprocess.CalledProcessError as error:
        if error.stderr:
            print(error.stderr, end="", file=sys.stderr)
        # Commands used for signing can contain credentials.
        parser.exit(1, f"{Path(error.cmd[0]).name} exited with status {error.returncode}\n")
    except (ValueError, OSError, tarfile.TarError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
