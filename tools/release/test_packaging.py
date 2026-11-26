import hashlib
import json
import os
import plistlib
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

from dependency_notices import copy_crate_notices
from packaging_release import ReleaseConfig, notice_digests, stage_payload, verify_notices, write_checksums


class PackagingTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.binary = self.root / "geokernel-app"
        self.binary.write_bytes(b"fixture executable")

    def test_macos_bundle_carries_metadata_and_notices(self):
        config = ReleaseConfig("1.0.0", "Example Geometry", "org.example.geometry", "both")
        app = self.root / "Example Geometry.app"
        stage_payload(config, "macos", "arm64", "eupl", self.binary, app)
        contents = app / "Contents"
        self.assertEqual((contents / "MacOS/geokernel-app").read_bytes(), self.binary.read_bytes())
        self.assertEqual((contents / "Resources/geokernel.icns").read_bytes()[:4], b"icns")
        with (contents / "Info.plist").open("rb") as plist:
            fields = plistlib.load(plist)
        self.assertEqual(fields["CFBundleIdentifier"], "org.example.geometry")
        self.assertEqual(fields["CFBundleShortVersionString"], "1.0.0")
        self.assertEqual(fields["CFBundleDisplayName"], "Example Geometry")
        self.assertIn("European Union Public Licence", (contents / "Resources/NOTICE").read_text())
        self.assertTrue((contents / "Resources/LICENSE").is_file())
        self.assertFalse((contents / "Resources/SOURCE-OFFER.txt").exists())

    def assert_desktop_notices(self, docs):
        repo = Path(__file__).resolve().parents[2]
        notice = (docs / "NOTICE").read_text(encoding="utf-8")
        self.assertIn("GeoGebra GmbH", notice)
        self.assertIn("https://github.com/jash90/geokernel", notice)
        self.assertIn("Ubuntu Light", notice)
        self.assertIn("Ubuntu-Font-Licence.txt", notice)
        self.assertEqual((docs / "LICENSE").read_bytes(), (repo / "LICENSE").read_bytes())
        font_license = docs / "licenses/fonts/Ubuntu-Font-Licence.txt"
        self.assertIn("UBUNTU FONT LICENCE", font_license.read_text(encoding="utf-8"))
        self.assertIn("Copyright 2011 Canonical Ltd.", notice)
        for filename in ("Hack-Regular.txt", "OFL.txt", "emoji-icon-font-mit-license.txt", "egui-icons.txt"):
            self.assertTrue(font_license.with_name(filename).is_file(), filename)
        with tarfile.open(docs / "geokernel-source.tar.gz") as archive:
            for filename in ("Cargo.toml", "Cargo.lock", "crates/geo-app/src/main.rs",
                             "LICENSE", "NOTICE", "tools/release/packaging_release.py"):
                blob = subprocess.check_output(["git", "show", f"HEAD:{filename}"], cwd=repo)
                with archive.extractfile(f"geokernel-source/{filename}") as source:
                    self.assertEqual(source.read(), blob, filename)

    def test_eupl_payloads_carry_source_and_all_font_notices(self):
        config = ReleaseConfig("1.0.0", "geokernel", "org.example.geokernel", "both")
        for platform, arch, docs in (
            ("macos", "arm64", "Contents/Resources"),
            ("windows", "x86_64", "."),
            ("linux", "x86_64", "usr/share/doc/geokernel"),
        ):
            with self.subTest(platform=platform):
                package = self.root / platform
                stage_payload(config, platform, arch, "eupl", self.binary, package)
                self.assert_desktop_notices(package / docs)

    def test_registry_notices_follow_build_artifacts(self):
        crate = self.root / "fixture-crate"
        crate.mkdir()
        manifest = crate / "Cargo.toml"
        manifest.write_text('[package]\nname = "fixture-crate"\nversion = "1.0.0"\nlicense = "MIT"\n')
        (crate / "LICENSE-MIT").write_text("fixture licence")
        messages = self.root / "build.jsonl"
        messages.write_text(json.dumps({
            "reason": "compiler-artifact",
            "package_id": "registry+https://github.com/rust-lang/crates.io-index#fixture-crate@1.0.0",
            "manifest_path": str(manifest),
        }) + "\n" + json.dumps({"reason": "build-finished", "success": True}) + "\n")
        dest = self.root / "notices"
        copy_crate_notices(messages, dest)
        self.assertIn("fixture-crate-1.0.0 | MIT", (dest / "COMPONENTS.txt").read_text())
        self.assertEqual((dest / "fixture-crate-1.0.0/LICENSE-MIT").read_text(), "fixture licence")

    def test_verified_payload_rejects_lost_font_licence(self):
        config = ReleaseConfig("1.0.0", "geokernel", "org.example.geokernel", "both")
        package = self.root / "installed"
        stage_payload(config, "windows", "x86_64", "eupl", self.binary, package)
        expected = notice_digests(package, "eupl")
        verify_notices(package, expected)
        (package / "licenses/fonts/Ubuntu-Font-Licence.txt").unlink()
        with self.assertRaisesRegex(ValueError, "Ubuntu-Font-Licence"):
            verify_notices(package, expected)

    def test_only_explicit_dispatch_can_publish(self):
        workflow = (Path(__file__).resolve().parents[2] / ".github/workflows/release.yml").read_text()
        triggers = workflow.split("\non:\n", 1)[1].split("\npermissions:", 1)[0]
        self.assertNotRegex(triggers, r"(?m)^  push:")
        self.assertRegex(triggers, r"(?m)^      publish:\n        description: .+\n        type: boolean\n        default: false$")
        jobs = workflow.split("\njobs:\n", 1)[1]
        release_command = jobs.index("gh release create")
        publish_job = jobs.rsplit("\n  publish:\n", 1)[1]
        self.assertGreaterEqual(release_command, jobs.index(publish_job))
        self.assertEqual(jobs.count("gh release create"), 1)
        self.assertIn(
            "\n    if: github.event_name == 'workflow_dispatch' && inputs.publish == true"
            " && needs.plan.outputs.publish == 'true'\n",
            publish_job,
        )

    def test_gpl_layout_carries_library_and_every_source_notice(self):
        config = ReleaseConfig("1.0.0", "geokernel", "org.example.geokernel", "both")
        sources = self.root / "sources"
        sources.mkdir()
        for name in ("giac-70507-source.archive", "giac-dependency-source.tar.gz", "GPL-3.0.txt"):
            (sources / name).write_bytes(name.encode())
        for platform, name, expect in (
            ("macos", "libjavagiac.jnilib", "Contents/Frameworks"),
            ("windows", "javagiac64.dll", "."),
            ("linux", "libjavagiac64.so", "usr/lib/geokernel-giac"),
        ):
            with self.subTest(platform=platform):
                library = self.root / name
                library.write_bytes(b"fixture library")
                package = self.root / platform
                binary = self.binary
                if platform == "windows":
                    binary = self.root / "geokernel-app.exe"
                    binary.write_bytes(self.binary.read_bytes())
                stage_payload(config, platform, "x86_64", "gpl", binary, package, sources, library)
                self.assertEqual((package / expect / name).read_bytes(), b"fixture library")
                notice, = package.rglob("NOTICE")
                self.assert_desktop_notices(notice.parent)
                offer = notice.with_name("SOURCE-OFFER.txt").read_text()
                self.assertIn("combined work", offer)
                self.assertIn("giac-dependency-source.tar.gz", offer)
                for item in (*sources.iterdir(), sources / "geokernel-source.tar.gz"):
                    self.assertTrue(notice.with_name(item.name).is_file(), item.name)
                digest = hashlib.sha256((sources / "giac-70507-source.archive").read_bytes()).hexdigest()
                self.assertIn(digest, notice.read_text())

    def test_gpl_refuses_missing_dependency_source(self):
        config = ReleaseConfig("1.0.0", "geokernel", "org.example.geokernel", "both")
        with self.assertRaises((ValueError, FileNotFoundError)):
            stage_payload(config, "linux", "x86_64", "gpl", self.binary, self.root / "app")

    def test_signed_macos_gpl_package_is_refused_before_staging(self):
        script = Path(__file__).with_name("packaging_release.py")
        output = self.root / "dist"
        result = subprocess.run(
            [sys.executable, str(script), "package", "macos", "x86_64", "gpl",
             str(self.binary), str(output), "--sign"],
            env=os.environ | {
                "RELEASE_VERSION": "1.0.0", "PRODUCT_NAME": "geokernel",
                "APP_IDENTIFIER": "org.example.geokernel", "LINUX_FORMATS": "both",
            },
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("the pinned Intel Giac library is unsigned", result.stderr)
        self.assertFalse(output.exists())

    def test_checksum_file_is_sorted_and_does_not_hash_itself(self):
        for name in ("z.pkg", "a.AppImage"):
            (self.root / name).write_bytes(name.encode())
        write_checksums(self.root)
        expected = "".join(
            f"{hashlib.sha256((self.root / name).read_bytes()).hexdigest()}  {name}\n"
            for name in ("a.AppImage", "geokernel-app", "z.pkg")
        )
        write_checksums(self.root)
        self.assertEqual((self.root / "SHA256SUMS").read_text(), expected)

    def test_invalid_release_settings_are_refused(self):
        for args in (
            ("1.0\n", "geokernel", "org.example.geokernel", "both"),
            ("1.0.0", "../bad", "org.example.geokernel", "both"),
            ("1.0.0", "geokernel", "org.example\n", "both"),
            ("1.0.0", "geokernel", "org.example.geokernel", "rpm"),
        ):
            with self.subTest(args=args), self.assertRaises(ValueError):
                ReleaseConfig(*args)


if __name__ == "__main__":
    unittest.main()
