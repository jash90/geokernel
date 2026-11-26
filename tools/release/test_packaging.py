import hashlib
import os
import plistlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from packaging_release import ReleaseConfig, stage_payload, write_checksums


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
