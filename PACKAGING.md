# Packaging

| Platform | Artifact | How | Verified in this session |
|---|---|---|---|
| Linux | Portable `.tar.gz` (+ `install.sh` per-user installer) | `packaging/linux/package.sh tarball` | **Yes:** built and checksummed locally (x86_64) |
| Linux | AppImage | `package.sh appimage` (needs `appimagetool`) | No: tool unavailable locally; built in CI |
| Linux | `.deb` | `cargo deb -p animara-app` (metadata in `crates/app/Cargo.toml`) | No: CI |
| Linux | `.rpm` | `cargo generate-rpm -p crates/app` | No: CI |
| Linux | Flatpak | `flatpak-builder … packaging/linux/io.github.animara.Animara.yml` | No: CI |
| Linux | Arch | `packaging/linux/PKGBUILD` | No |
| Windows | `.msi` (WiX v4: Start menu, optional desktop shortcut, `.anim` association, PATH for CLI, major upgrades, uninstall) | `wix build packaging/windows/animara.wxs …` | No: CI |
| Windows | Setup `.exe` (NSIS: components page, desktop shortcut option, association, uninstaller) | `makensis packaging/windows/animara.nsi` | No: CI |
| Windows | Portable `.zip`, standalone `.exe` | release workflow | No: CI |
| macOS | `.app`, `.dmg`, `.pkg`, `.tar.gz` (universal x86_64+arm64) | `UNIVERSAL=1 packaging/macos/package.sh` | No: CI |
| macOS | Terminal install | `packaging/macos/install.sh` (download + SHA-256 verify + install) | No |
| macOS | Homebrew | `packaging/macos/animara.rb` cask template for a tap | No |

Every release artifact name includes version, platform and architecture, e.g.
`animara-0.1.0-linux-x86_64.tar.gz` or `Animara-0.1.0-windows-x64.msi`. The publish job writes `SHA256SUMS.txt`
and release notes taken from the latest CHANGELOG section.

## Signing

* **macOS:** set `MACOS_SIGN_IDENTITY`, `MACOS_INSTALLER_IDENTITY`, `APPLE_ID`, `APPLE_TEAM_ID` and
  `APPLE_APP_PASSWORD` repository secrets to enable `codesign`, `notarytool` and `stapler`. Without them,
  artifacts are unsigned, and users must right-click ▸ Open the first time.
* **Windows:** no Authenticode certificate is configured. To sign, add a `signtool sign` step after the MSI/EXE
  steps with your certificate. Unsigned installers trigger SmartScreen warnings.

No installer has been run on Windows or macOS in this session. Treat those packages as untested until the release
workflow has executed and its smoke tests pass.
