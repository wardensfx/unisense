# unisense — resumption notes for an agent/dev

Windows tool (Rust) that normalizes mouse aim sensitivity across games via
the **Interception** driver. Full user documentation (installation, config,
licenses, computing constants) is in `README.md` — this file is for
whoever (human or agent) picks up the **code**, not the usage.

## State as of the last commit

Builds and tests on this dev machine (rustc 1.97, toolchain
`stable-x86_64-pc-windows-msvc`, installed via `winget install
Rustlang.Rustup`). `cargo build --workspace`, `cargo test --workspace`, and
`cargo build --workspace --release` all pass, 7 unit tests OK.

What has **not** been tested, for lack of means in the environment where
the code was written: the Interception driver isn't installed here (no
`.sys` loaded), so no test with a real mouse/a real game. The GUI was
verified via screenshot (PrintWindow) and simulated clicks, not by a human.
The sensitivity constants in `config/games.example.yaml` are "widely cited"
but not re-verified against a primary source for most games (see the
warnings in the YAML itself and the README).

## Useful commands

Rust is installed via rustup in `~/.cargo` but **not on the PATH** by
default in this shell — prefix commands with:
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

- `cargo build --workspace` / `cargo test --workspace`: build/test all 3 members.
- `cargo build --release -p unisense`: just the tray app (fast).
- `cargo build --release -p unisense-gui`: just the GUI (slower, pulls in Tauri + WebView2).
- `cargo build --release` (no `-p`): builds all three — the slowest option.
- No `tauri-cli` installed or needed: `cargo build`/`cargo run` on
  `unisense-gui` are enough (no `cargo tauri dev/build`).

## Structure (Cargo workspace, 3 members)

- `core/` — `unisense-core` lib: config schema (serde) + cm/360 factor
  calculation (`scaling.rs`, tested). **Zero Win32 dependency** here,
  deliberately kept portable so it stays shareable between `app` and `gui`.
- `app/` — the tray executable (`unisense.exe`). Interception capture,
  keyboard hotkeys (RegisterHotKey) / mouse hotkeys (in the Interception
  stream itself), pure Win32 systray (no GUI framework, a single invisible
  window + message loop).
- `gui/src-tauri/` + `gui/frontend/` — Tauri v2 configuration GUI. **Vanilla
  HTML/CSS/JS frontend, no bundler or npm** (static files served directly
  by Tauri). `withGlobalTauri: true` in `tauri.conf.json` to get
  `window.__TAURI__` without an ES module import.
- `vendor/interception/` — user-mode DLL + driver installer, redistributed
  as-is (LGPL 3.0, see `NOTICE.md` inside). `app/build.rs` copies the DLL
  next to the exe on every build (see OUT_DIR ancestors(3) to find
  `target/<profile>/`).

## Decisions and why (so they don't get redone / re-litigated)

- **Dynamic loading of `interception.dll`** via `libloading`, no static
  linking against `interception.lib`: avoids needing the SDK at build time,
  and lets a user replace the DLL without recompiling unisense — important
  for LGPL compliance (see `vendor/interception/NOTICE.md`), not just a
  convenience.
- **Tauri rather than egui** for the GUI: an explicit choice made by the
  user (compared via AskUserQuestion), at the cost of a WebView2 dependency
  (present by default on Windows 11) in exchange for a higher visual
  ceiling than pure-Rust immediate-mode.
- **Interception's license**: dual LGPL 3.0 (non-commercial use) /
  separate commercial license — verified by actually reading the license
  files in the upstream repo (`licenses/non-commercial-usage/LGPL
  3.0.txt`), not assumed upfront. My first pass at the README had
  incorrectly claimed "BSD-3-Clause" from memory; corrected after
  verification. **Always verify a third-party binary's exact license
  before vendoring it**, don't guess.
- **Auto-detection (5.2) with no pre-filled offsets**: deliberately left
  empty for every game in the example config — I don't have verified
  memory offsets for any given game, and shipping fake ones would be
  misleading. The memory-scan assistant in the GUI exists so the user can
  find their own.

## CSS pitfall hit (so it doesn't happen again)

In `gui/frontend/style.css`, rules like
`.drawer/.games-grid/.empty-state { display: flex|grid }` have the **same
specificity** as `[hidden]` and win the cascade (author rule beats UA rule
at equal specificity) → the `hidden` attribute was being silently ignored
(the edit drawer stayed visible at all times right from load). Fixed with a
global rule at the top of the file:
```css
[hidden] { display: none !important; }
```
If a new element toggled via `el.hidden = true/false` in JS doesn't hide,
this is probably the pitfall resurfacing.

## Verification done this session (to calibrate confidence)

Rust wasn't installed at the start of the session; installed via winget to
actually compile/test instead of shipping unverified code. Every compile
error hit (Win32 bindings, macros, BOOL/pointer coercions) was fixed by
looking at the compiler's actual messages, not guessed. The GUI was
verified via real screenshots (PrintWindow + simulated clicks), not just
"this should work."

## Release pipeline (automatic semver)

Adapted from `OuyouyouTube` (same release-please pattern), see
`.github/workflows/{ci,release-please,publish-release-assets}.yml`,
`.github/{release.yml,dependabot.yml}`, `release-please-config.json`,
`.release-please-manifest.json`.

- **`ci.yml`**: build+test+fmt+clippy on `windows-latest` (the code doesn't
  compile on Linux/macOS, it's Win32/WebView2). `cargo fmt --check` and
  `cargo clippy -- -D warnings` are **blocking** — the workspace was
  fmt/clippy-clean at the time this pipeline was added, so it should never
  fail on code that was already cleanly mergeable.
- **`release-please.yml`**: runs on push to `main`, reads **Conventional
  Commits** (`feat:`, `fix:`, `docs:`, `chore:`, etc.) since the last tag,
  maintains a release PR with an up-to-date `CHANGELOG.md` + version bump,
  and creates the tag + GitHub Release once that PR is merged. **The
  version being bumped is `workspace.package.version` in the root
  Cargo.toml** (TOML extra-file in `release-please-config.json`), which
  cascades to all 3 members via `version.workspace = true` — no per-crate
  version to maintain separately.
- **`publish-release-assets.yml`**: dispatched by `release-please.yml`
  (not by the `release: published` event — `GITHUB_TOKEN` doesn't
  re-trigger other workflows, same reasoning as in OuyouyouTube). Builds a
  release, zips `unisense.exe` + `interception.dll` + `unisense-gui.exe` +
  `config/` + `vendor/` + README/LICENSE, attaches it to the GitHub
  Release.
- **Important: commits must follow Conventional Commits** from now on for
  the version bump to be correct (`feat:` -> minor, `fix:` -> patch,
  `feat!:`/`BREAKING CHANGE:` -> major, `chore:`/`ci:`/`test:`/`build:` ->
  no bump, just tucked into the changelog). Commits from before this
  pipeline was added didn't follow this convention — no consequence, they're
  already "inside" the manifest's 0.1.0 baseline; release-please only looks
  at what comes after.

**GitHub-side setup**: the remote is `git@github.com:wardensfx/unisense.git`.
Once pushed with Actions enabled, check: Settings > Actions > General >
"Allow GitHub Actions to create and approve pull requests" must be checked,
otherwise release-please can't open its PR.

## Not done yet / open threads

See the README's "Ideas for improvement" section (foreground-process
detection, YAML hot-reload, on-screen status overlay, multiple mice,
installer, local API, pointer chain in the GUI, native file picker). None
of it has been started.
