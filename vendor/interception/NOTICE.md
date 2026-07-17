# Provenance and license of the files in this folder

These files are redistributed as-is from the official `v1.0.1` release of
the **Interception** project:

- Repository: https://github.com/oblitum/Interception
- Release: https://github.com/oblitum/Interception/releases/tag/v1.0.1
- Source file: `library/x64/interception.dll` (and `command line
  installer/install-interception.exe`) from the `Interception.zip` archive
  published at that release.

None of these binaries have been modified.

## License

Interception is dual-licensed:

- **Commercial use**: separate license, see `licenses/commercial-usage/`
  in the upstream repo (not applicable here).
- **Non-commercial use**: **LGPL 3.0**, see
  `licenses/non-commercial-usage/LGPL 3.0.txt` in the upstream repo (full
  text reproduced in `LGPL 3.0.txt` next to this file).

unisense is a free, non-commercial tool (MIT license, see `LICENSE` at the
repo root): the **LGPL 3.0** track is therefore what applies to the
redistribution of `interception.dll` done here.

`interception.dll` remains a distinct LGPL 3.0 component, separate from
unisense's own (MIT) source code. Per the LGPL:

- The corresponding source is publicly and durably available at the
  address above (official upstream repo, release v1.0.1).
- unisense loads `interception.dll` **dynamically at runtime**
  (`libloading`, no static linking / no `interception.lib` embedded in
  unisense's binary): a user can therefore replace this file with their
  own (recompiled or modified) version of the library without having to
  recompile unisense, which satisfies the LGPL's "library replacement"
  requirement.

The kernel driver (`install-interception.exe`, which installs the `.sys`)
is included here purely for install convenience; see the README at the
repo root for the procedure.
