# rogg — CI builds on push to main — Design

Date: 2026-10-10
Status: Approved in conversation; written for review
Builds on: `2026-10-09-rogg-cross-platform-design.md`, at `main` 3815cf6
Repository: `github.com/rogg-log/rogg` (private for now)

## 1. Intent

Every push to `main` builds rogg for macOS, Linux and Windows on GitHub Actions. The builds are published at stable download URLs, so a future `install.sh` and an in-app updater can fetch them.

**Success means all of the following:**
1. A push to `main` runs the workflow. If the tests pass and all four builds succeed, the `main-latest` prerelease holds the four archives, `SHA256SUMS` and `manifest.json` for that commit.
2. Every archive contains a binary that starts on its platform:
   - macOS universal (Apple silicon and Intel, macOS 11 or newer);
   - Linux x86_64 and arm64 (glibc 2.35 or newer);
   - Windows x86_64 (a release build with no console window).
3. The binary knows its build number and channel, and the About dialog shows them.
4. Local builds and the macOS goldens are unchanged.

## 2. Decisions (agreed in conversation)

| Topic | Decision |
|---|---|
| Where builds go | One rolling prerelease, tag `main-latest`, replaced on every push. Old builds are not kept. Workflow artifacts are used only to hand files from the build jobs to the publish job. |
| Repository visibility | Private for now. Download URLs need a token until the repo is public; nothing in the design changes when it becomes public. |
| Targets | macOS universal (one job), Linux x86_64, Linux arm64, Windows x86_64. |
| Tests in CI | One Linux job runs clippy and the full test suite and gates publishing. visual-check stays local (it needs a Mac GPU and Playwright). |

## 3. Workflow — `.github/workflows/build.yml`

**Triggers:** `push` to `main`, and `workflow_dispatch`.
**Concurrency:** group `build-main` with `cancel-in-progress: true`, so a newer push cancels an older run.
**Permissions:** `contents: read` by default; the publish job alone gets `contents: write`.
**Actions:** only GitHub's own actions (`actions/checkout`, `actions/cache`, `actions/upload-artifact`, `actions/download-artifact`), pinned to full commit SHAs with the version in a comment, plus the `gh` CLI that is preinstalled on the runners.
**Toolchain:** rustup is preinstalled on the runners, and `rust-toolchain.toml` installs the pinned 1.99.0 (with clippy and rustfmt) on first use.
**Caching:** `actions/cache` keeps `~/.cargo/registry`, `~/.cargo/git` and `target/`, keyed by job, OS and the hash of `Cargo.lock` and `rust-toolchain.toml`, with a fallback to the job's latest cache.

| Job | Runner | Steps |
|---|---|---|
| `check` | `ubuntu-22.04` | Installs gpui's Linux build dependencies (the `-dev` packages from `tools/linux/Containerfile`), then runs `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`. |
| `macos` | `macos-14` | Adds the `x86_64-apple-darwin` target, then builds `--release -p rogg-app` for `aarch64-apple-darwin` and `x86_64-apple-darwin` with `MACOSX_DEPLOYMENT_TARGET=11.0`. It joins the two with `lipo -create`, re-signs with `codesign --force --sign -`, checks that `lipo -archs` gives `x86_64 arm64` and that `codesign --verify` passes, then packages. |
| `linux-x86_64` | `ubuntu-22.04` | Installs the dependencies, runs `cargo build --release -p rogg-app`, then packages. |
| `linux-arm64` | `ubuntu-22.04-arm` | The same steps as `linux-x86_64`. |
| `windows-x86_64` | `windows-latest` | Runs `cargo build --release -p rogg-app` (a native release build, so the shader compiler is available and `windows_subsystem = "windows"` applies), then packages. |
| `publish` | `ubuntu-22.04` | Needs every job above. Downloads the four archives, writes `SHA256SUMS` and `manifest.json`, then updates the release (§5). |

Every build job sets `ROGG_BUILD_NUMBER=${{ github.run_number }}` and `ROGG_BUILD_CHANNEL=main`, and uploads its archive as a workflow artifact for the publish job (retention 1 day).

**Risk:** GitHub's arm64 Linux runners (`ubuntu-22.04-arm`) are free for public repositories, but their availability for private repositories depends on the plan. If the first run shows the runner is unavailable, the `linux-arm64` job is removed until the repository is public, and `publish` stops depending on it. That is a recorded decision, not a silent skip.

**Cost:** while the repo is private, the macOS job is billed at 10× and the Windows job at 2× the Linux rate. Caching and cancelling older runs keep this down. Narrowing the trigger is a later option if minutes run short.

## 4. Archives — `tools/ci/package.sh`

`package.sh <os> <arch> <binary>` writes `dist/rogg-<os>-<arch>.<ext>`. It contains:
- the binary (`rogg`, or `rogg.exe` on Windows);
- `README.md`;
- `crates/rogg-ui/assets/fonts/Inter-OFL.txt` and `JetBrainsMono-OFL.txt`. The OFL requires the licence to travel with the fonts, which are embedded in the binary.

The archives are:
- `rogg-macos-universal.tar.gz`
- `rogg-linux-x86_64.tar.gz`
- `rogg-linux-arm64.tar.gz`
- `rogg-windows-x86_64.zip`

Files sit at the archive root, not in a folder. Names carry no version, so URLs stay stable.

The script is `bash`, so it runs on all runners (Git Bash on Windows) and on the Mac for local testing. `.tar.gz` archives use `tar`. `.zip` archives use `zip` where it exists, else `7z`, which is preinstalled on GitHub's Windows runners. The script fails if any input file is missing.

## 5. Publishing — `tools/ci/manifest.sh` and the `publish` job

`manifest.sh <dir> <build> <commit> <channel>` writes `SHA256SUMS` (in `sha256sum` format) and `manifest.json` in `<dir>` from the archives there:

```json
{
  "channel": "main",
  "build": 123,
  "commit": "<full sha>",
  "date": "<UTC ISO-8601>",
  "assets": [
    { "name": "rogg-macos-universal.tar.gz", "os": "macos", "arch": "universal", "sha256": "<hex>", "size": 12345678 }
  ]
}
```

`os` and `arch` come from the archive name. The assets are listed in name order, and the script uses only `bash`, `sha256sum`/`shasum` and `stat`, with no `jq`. It fails if an archive name doesn't match `rogg-<os>-<arch>.<ext>`.

**The publish job** uses `gh` with `GH_TOKEN: ${{ github.token }}`:
1. If the release `main-latest` doesn't exist, create it as a prerelease on the pushed commit.
2. Move the tag `main-latest` to the pushed commit (`git tag -f`, then a forced push of the tag only).
3. Set the title to `Build <n> · <short sha>` and the notes to the commit subject plus a pointer to `manifest.json`.
4. Upload the archives with `--clobber`, then `SHA256SUMS`, then `manifest.json` **last**.
5. Delete any release asset that is not in the new set.

**Consistency:** during an upload, a reader may briefly see new archives next to the old manifest. Consumers always verify the SHA-256 from the manifest they read. A mismatch means "retry later", never "install". The manifest goes up last, so a consistent manifest always describes files that are already uploaded.

**Stable URLs:** `https://github.com/rogg-log/rogg/releases/download/main-latest/<file>`.

## 6. Build number in the app

- `crates/rogg-app/build.rs` passes through `ROGG_BUILD_NUMBER` and `ROGG_BUILD_CHANNEL` when they are set (`cargo:rustc-env`), and declares `cargo:rerun-if-env-changed` for both.
- `about.rs` `BuildInfo` gains `build` and `channel`, and the About dialog shows `build <n> (<channel>)`. A local build shows `local` in place of the build number and channel.
- The About dialog has no golden, so the goldens are unaffected.
- A unit test covers `describe` with and without the two values.

## 7. Verification

1. `./dev check` stays green on macOS (goldens at 0.000%).
2. The workflow is linted locally with `actionlint` from its official container image (`rhysd/actionlint`) through Apple `container`. Nothing is installed on the Mac. A new `./dev ci-lint` runs it.
3. `tools/ci/package.sh` and `manifest.sh` are tested locally against fake binaries. A new `./dev ci-test` script checks:
   - the archive names and contents;
   - that `SHA256SUMS` verifies;
   - that `manifest.json` is valid JSON with the expected fields (checked with Python's `json` module, which is present on macOS);
   - that a bad archive name or a missing file fails.
4. Pushing the workflow to `main` is the real test. **It is an outward action: ask the user before pushing.** Then watch the run (with the user, or through the GitHub web UI if `gh` is not installed locally) and fix what breaks. Fixes go in follow-up commits on `main`, each pushed only after asking.

## 8. Out of scope

- `install.sh`, the in-app updater, and `.app`/`.dmg` bundles.
- Developer ID signing, notarisation and Windows code signing.
- Versioned or tagged releases, and changelogs.
- Running tests on macOS or Windows in CI, and visual-check in CI.
- A project licence file (the user's decision; the archives carry the README and the font licences).

## 9. Amendment (2026-10-10): macOS app bundle alongside the binary

Approved in conversation: keep the bare macOS binary, and add a `rogg.app` built from that same binary. Double-clicking a bare executable in Finder always runs it through Terminal; a `.app` launches directly.

**No change to the Rust build.** The bundle is a packaging step after the universal binary is made (§3, `macos` job): `tools/ci/macos-app.sh <binary>` writes `$DIST_DIR/rogg-macos-universal-app.tar.gz`, whose root is `rogg.app/`:

```
rogg.app/Contents/Info.plist
rogg.app/Contents/PkgInfo                      APPL????
rogg.app/Contents/MacOS/rogg                   the same universal binary
rogg.app/Contents/Resources/README.md, Inter-OFL.txt, JetBrainsMono-OFL.txt, rogg.icns
```

**Info.plist:**

| Key | Value |
|---|---|
| `CFBundleName`, `CFBundleDisplayName` | `rogg` |
| `CFBundleIdentifier` | `io.rogg-log.rogg` (changing it later resets users' macOS permission grants) |
| `CFBundleExecutable` | `rogg` |
| `CFBundlePackageType` | `APPL` |
| `CFBundleShortVersionString` | the workspace version from `Cargo.toml` (`0.1.0`) |
| `CFBundleVersion` | `ROGG_BUILD_NUMBER`, or `0` for a local build |
| `LSMinimumSystemVersion` | `11.0` |
| `NSHighResolutionCapable` | `true` |
| `LSApplicationCategoryType` | `public.app-category.developer-tools` |

**Icon** (approved 2026-10-10, concept "the match": dim log lines, one highlighted in rogg's amber): the source is `crates/rogg-app/assets/icon/rogg.svg`, plus `rogg-small.svg` (three thicker lines) for the 16 and 32 px renders. `./dev icon` (`tools/icon/make-icns.mjs`: Playwright Chromium renders, then `iconutil`) regenerates the committed `rogg.icns`. `macos-app.sh` copies it to `Contents/Resources/rogg.icns`, and `Info.plist` gains `CFBundleIconFile` = `rogg`.

**Signing:** after assembling, `codesign --force --sign - rogg.app` (ad-hoc, covering `Info.plist` and resources), then `codesign --verify --deep --strict`. Where `codesign` is missing (the Linux `check` job runs the script's tests), the script skips signing and says so. Gatekeeper behaviour is the same as for the bare binary: no prompt when not quarantined (`curl`, `install.sh`); "Open Anyway" when downloaded by a browser.

**Names and manifest:**
- The archive name scheme becomes `rogg-<os>-<arch>[-app].<ext>`. The `-app` suffix is allowed only for macOS `.tar.gz`. `manifest.sh` rejects anything else.
- Every manifest asset gains `"kind"`: `"app"` for the `-app` archive, `"binary"` for all others. Existing names and URLs are unchanged, and nothing reads the manifest yet, so the extra field breaks nothing.
- `publish.sh` already uploads every `rogg-*` archive. The release grows to 7 assets.

**Workflow:** the `macos` job runs `macos-app.sh` after `package.sh` and uploads both archives in its artifact.

**Local:** `./dev mac-app` builds a release binary for this Mac, then `rogg.app` in `target/mac-app/`, to try in Finder. `./dev bundle` is taken by the sample-bundle generator.

**Tests** (`tools/ci/test.sh`, run by `./dev ci-test` and by the `check` job):
- the app archive's exact listing;
- `Info.plist` values, parsed with Python's `plistlib`;
- the executable bit on `Contents/MacOS/rogg`;
- `codesign --verify --deep --strict` where `codesign` exists;
- the manifest's `kind` for each asset;
- `-app` rejected for Linux, Windows and `.zip`.

**Out of scope:** a `.dmg`, Developer ID signing and notarisation, file associations.
