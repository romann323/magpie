# Build and release

## Prerequisites (Windows)

```powershell
# Rust toolchain (user scope)
winget install --id Rustlang.Rustup --accept-source-agreements

# MSVC C++ Build Tools (machine scope; needs UAC)
winget install --id Microsoft.VisualStudio.2022.BuildTools `
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# Node LTS
winget install --id OpenJS.NodeJS.LTS
```

Verify:

```powershell
rustc --version   # ≥ 1.77.2
cargo --version
node --version    # ≥ 18
npm --version
```

## Clone and build

```powershell
git clone <repo> picorg
cd picorg
npm install
```

### Development

```powershell
npm run tauri dev
```

This starts:

- **Vite dev server** on `localhost:1420` with HMR.
- **Cargo watch** compiling the Rust core.
- **Tauri window** loading the dev server URL.

Rust code changes trigger a full app rebuild (10–30 s); frontend
changes hot-reload in <1 s.

### Release build

```powershell
npm run tauri build -- --no-bundle
```

- `--no-bundle` skips the MSI/NSIS installer, producing just
  `src-tauri/target/release/picorg.exe`. Faster and useful for
  local testing.
- Remove `--no-bundle` (or pass `--bundles msi` / `--bundles nsis`)
  to produce an installer under `src-tauri/target/release/bundle/`.

Timings on a modern laptop:

- Cold release build: ~5 min.
- Incremental release build after a small Rust change: ~90 s.
- Incremental after frontend change only: ~30 s.

## Tauri configuration

Key fields in `src-tauri/tauri.conf.json`:

| Field                    | Notes                                                    |
| ------------------------ | -------------------------------------------------------- |
| `productName`            | `PicOrg`                                                 |
| `identifier`             | `com.picorg.picorg` — used for `%APPDATA%` folder name.  |
| `security.csp`           | Strict; blocks inline scripts and remote origins.        |
| `app.withGlobalTauri`    | `false` — commands accessed via `@tauri-apps/api` only.  |
| `bundle.icon`            | Multi-size Windows ICO.                                  |
| `bundle.windows.wix`     | (Present in future for signed MSI.)                      |

Capabilities (`src-tauri/capabilities/default.json`) declare only
what's needed: dialog and opener plugins, asset access under the
user's library folders. Network is *not* granted.

## Release profile

`src-tauri/Cargo.toml`:

```toml
[profile.release]
codegen-units = 16
lto           = false
opt-level     = 3
panic         = "abort"
strip         = true
incremental   = false
```

Rationale:

- `codegen-units = 16` + `lto = false` — trades a bit of runtime
  perf for much faster incremental builds. In practice, the hot
  paths are limited by `image` decode and disk I/O, not by
  cross-crate inlining.
- `panic = "abort"` — smaller binary, no unwinding tables. All
  panics are treated as bugs; the app crashes and restarts.
- `strip = true` — removes debug symbols to keep the binary small
  (~13 MB stripped vs. 45 MB with symbols).

## Version bump

1. `Cargo.toml`: `version = "0.1.1"` (both files if we ever add a
   workspace).
2. `package.json`: same.
3. `tauri.conf.json`: `version = "0.1.1"`.
4. Commit as `chore: v0.1.1`.

## Releasing

The v1 workflow is manual:

1. `npm run tauri build` (with default bundle) on a clean tree.
2. Test the resulting MSI on a clean VM.
3. Draft a GitHub release with the MSI attached.
4. Attach the two PDFs from `docs/` as documentation assets.
5. Tag the commit `v0.1.0` and publish.

A GitHub Actions workflow is not shipped in v1 but can plug into
the recommended CI setup in [Testing strategy](./testing.md#ci-recommended).
