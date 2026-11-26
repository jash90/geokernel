# Native release inputs

Pull requests and manual dispatches with the default `publish: false`
produce workflow artifacts. Publishing requires a manual dispatch with
`publish: true` and an existing version tag pointing at the exact packaged
commit. Pushing a tag does not trigger the workflow.

| Download | Pin and verification |
| --- | --- |
| Ubuntu 24.04 Linux build image | OCI SHA-256 digest in [build-linux.sh](build-linux.sh) |
| Ubuntu snapshot HTTPS bootstrap CA bundle | Noble `ca-certificates 20260816ubuntu0.24.04.2` and SHA-256 in [build-linux.sh](build-linux.sh) |
| Linux system packages | Ubuntu snapshot `20261125T000000Z` in [build-linux.sh](build-linux.sh). APT verifies signed Ubuntu indexes and indexed package hashes. The install list and its snapshot-resolved dependencies are build/test inputs. |
| Rust compiler, Cargo and host standard library | Rustup's minimal profile for `1.100.0` in [release.yml](../../.github/workflows/release.yml). Rustup verifies component archives against the upstream release manifest for the host triple. The project pins the toolchain version and relies on upstream for the component checksums. |
| Cargo registry crates | Exact versions and archive SHA-256 checksums in [Cargo.lock](../../Cargo.lock), enforced by Cargo with `--locked` |
| appimagetool | `1.9.1` x86_64 AppImage and SHA-256 in [packaging_release.py](packaging_release.py) |
| AppImage type-2 runtime | `20251108` x86_64 runtime and upstream source archive, each with its own SHA-256 in [packaging_release.py](packaging_release.py) |
| libfuse source for AppImage runtime | `3.15.0` publisher archive and SHA-256 in [packaging_release.py](packaging_release.py) |
| Inno Setup | `6.5.4` installer and SHA-256 in [packaging_release.py](packaging_release.py) |
| javagiac native JAR and member | `70507`, with separate SHA-256 values per platform and archive member in [fetch.sh](../giac/fetch.sh) |
| Giac publisher source | HTTPS dispatch input or repository variable with the SHA-256 hardcoded in [fetch-source.sh](../giac/fetch-source.sh) |
| GMP/MPFR source for Giac | Owner-reviewed HTTPS bundle and SHA-256 in the repository variables consumed by [release.yml](../../.github/workflows/release.yml) |
| GPL 3.0 licence text for Giac | GNU-published text and SHA-256 in [packaging_release.py](packaging_release.py) |
| GitHub Actions checkout, upload-artifact and download-artifact | Full action commit hashes in [release.yml](../../.github/workflows/release.yml) |
| Package transfer between jobs | Artifacts from the current workflow run. The collection and publish jobs recheck `SHA256SUMS` before upload. |

The Windows SDK, Apple SDK and platform packaging tools are supplied by
the GitHub-hosted runner images. Signed builds also use the platforms'
timestamping or notarization services.

## Attribution inputs

The notices under `licenses/fonts` are from `epaint_default_fonts 0.37.0`
in Cargo.lock. The Ubuntu Font Licence replaces the upstream filename
`UFL.txt` in the payload. Font files are embedded by Cargo dependencies.

The crate notices are collected from Cargo's compiler artifacts for each
native build. The `crate-fallbacks` directory provides repository-level
notices for crates whose published archives omit them. It draws from
egui 0.37.0, AccessKit revision `f302622`, objc2 revision `d7d2fa2`,
rspirv revision `8afc3d0` and SPIRV-Headers SDK 1.4.341.0, profiling
revision `8271551`, clipboard-win revision `ed27008` and gl-rs revision
`ea503e8`. The dispatch notice uses the MIT licence and attribution from
its published Cargo metadata. Rust notices come from the build toolchain. The AppImage and
Windows installer add the notices for their respective packaging runtime.
