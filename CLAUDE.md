# unisense — notes de reprise pour agent/dev

Outil Windows (Rust) qui normalise la sensibilite de visee souris entre jeux
via le driver **Interception**. La doc utilisateur complete (installation,
config, licences, calcul des constantes) est dans `README.md` — ce fichier
est pour quelqu'un (humain ou agent) qui reprend le **code**, pas l'usage.

## Etat au dernier commit

Compile et teste sur cette machine de dev (rustc 1.97, toolchain
`stable-x86_64-pc-windows-msvc`, installee via `winget install
Rustlang.Rustup`). `cargo build --workspace`, `cargo test --workspace` et
`cargo build --workspace --release` passent tous, 7 tests unitaires OK.

Ce qui n'a **pas** ete teste, faute de moyens dans l'environnement ou le
code a ete ecrit : le driver Interception n'est pas installe ici (pas de
`.sys` charge), donc pas de test avec une vraie souris/un vrai jeu. La GUI a
ete verifiee par capture d'ecran (PrintWindow) et clics simules, pas par un
humain. Les constantes de sensibilite dans `config/games.example.yaml` sont
"largement citees" mais pas re-verifiees aupres d'une source primaire pour
la plupart des jeux (voir avertissements dans le YAML lui-meme et le README).

## Commandes utiles

Rust est installe via rustup dans `~/.cargo` mais **pas dans le PATH** par
defaut de ce shell — prefixer les commandes :
```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

- `cargo build --workspace` / `cargo test --workspace` : build/test les 3 membres.
- `cargo build --release -p unisense` : juste l'app tray (rapide).
- `cargo build --release -p unisense-gui` : juste la GUI (plus lent, tire Tauri + WebView2).
- `cargo build --release` (sans `-p`) : build les trois — le plus lent.
- Pas de `tauri-cli` installe ni necessaire : `cargo build`/`cargo run` sur
  `unisense-gui` suffisent (pas de `cargo tauri dev/build`).

## Structure (workspace Cargo, 3 membres)

- `core/` — lib `unisense-core` : schema de config (serde) + calcul du
  facteur cm/360 (`scaling.rs`, teste). **Zero dependance Win32** ici,
  gardee portable expres pour rester partageable entre `app` et `gui`.
- `app/` — l'executable tray (`unisense.exe`). Capture Interception,
  hotkeys clavier (RegisterHotKey) / souris (dans le flux Interception lui
  meme), systray en Win32 pur (pas de framework GUI, une seule fenetre
  invisible + message loop).
- `gui/src-tauri/` + `gui/frontend/` — GUI Tauri v2 de configuration.
  Frontend HTML/CSS/JS **vanilla, sans bundler ni npm** (fichiers statiques
  servis directement par Tauri). `withGlobalTauri: true` dans
  `tauri.conf.json` pour avoir `window.__TAURI__` sans import ES module.
- `vendor/interception/` — DLL utilisateur + installeur du driver,
  redistribues tels quels (LGPL 3.0, cf `NOTICE.md` dedans). `app/build.rs`
  copie le DLL a cote de l'exe a chaque build (voir OUT_DIR ancestors(3)
  pour trouver `target/<profile>/`).

## Decisions et pourquoi (pour ne pas les redefaire / redebattre)

- **Chargement dynamique de `interception.dll`** via `libloading`, pas de
  lien statique contre `interception.lib` : evite d'avoir besoin du SDK au
  moment du build, et permet a un utilisateur de remplacer le DLL sans
  recompiler unisense — important pour la conformite LGPL (cf
  `vendor/interception/NOTICE.md`), pas juste une commodite.
- **Tauri plutot qu'egui** pour la GUI : choix explicite fait par
  l'utilisateur (compare via AskUserQuestion), au prix d'une dependance
  WebView2 (presente par defaut sur Windows 11) contre un plafond
  esthetique plus haut que de l'immediate-mode Rust pur.
- **Licence d'Interception** : dual LGPL 3.0 (usage non commercial) /
  licence commerciale separee — verifie en allant lire les fichiers de
  licence du depot amont (`licenses/non-commercial-usage/LGPL 3.0.txt`),
  pas suppose a priori. Ma premiere version du README avait affirme a tort
  "BSD-3-Clause" de memoire ; corrige apres verification. **Toujours
  verifier la licence exacte d'un binaire tiers avant de le vendoriser**,
  ne pas la deviner.
- **Detection auto (5.2) sans offsets pre-remplis** : deliberement laisse
  vide pour tout jeu dans la config d'exemple — je n'ai pas d'offsets
  memoire verifies pour un jeu quelconque, en fournir de faux serait
  trompeur. L'assistant de scan memoire dans la GUI sert a ce que
  l'utilisateur les trouve lui-meme.

## Piege CSS rencontre (pour ne pas le refaire)

Dans `gui/frontend/style.css`, des regles comme
`.drawer/.games-grid/.empty-state { display: flex|grid }` ont la **meme
specificite** que `[hidden]` et gagnent la cascade (regle d'auteur bat regle
UA a specificite egale) → l'attribut `hidden` etait silencieusement ignore
(le tiroir d'edition restait visible en permanence des le chargement).
Corrige par une regle globale en tete de fichier :
```css
[hidden] { display: none !important; }
```
Si un nouvel element toggle via `el.hidden = true/false` en JS ne se cache
pas, c'est probablement ce piege qui revient.

## Verification faite cette session (pour calibrer la confiance)

Rust n'etait pas installe au debut de la session ; installe via winget pour
pouvoir reellement compiler/tester au lieu de livrer du code non verifie.
Toutes les erreurs de compilation rencontrees (bindings Win32, macros,
coercions BOOL/pointeurs) ont ete corrigees en observant les vrais messages
du compilateur, pas devinees. La GUI a ete verifiee par capture d'ecran
reelle (PrintWindow + simulation de clics), pas juste "ca devrait marcher".

## Pipeline de release (semver automatique)

Adapte de `OuyouyouTube` (meme pattern release-please), voir
`.github/workflows/{ci,release-please,publish-release-assets}.yml`,
`.github/{release.yml,dependabot.yml}`, `release-please-config.json`,
`.release-please-manifest.json`.

- **`ci.yml`** : build+test+fmt+clippy sur `windows-latest` (le code ne
  compile pas sur Linux/macOS, c'est du Win32/WebView2). `cargo fmt --check`
  et `cargo clippy -- -D warnings` sont **bloquants** — le workspace etait
  fmt/clippy-clean au moment ou ce pipeline a ete ajoute, donc ca ne devrait
  jamais echouer sur du code deja mergeable proprement.
- **`release-please.yml`** : tourne sur push vers `main`, lit les commits
  **Conventional Commits** (`feat:`, `fix:`, `docs:`, `chore:`, etc.) depuis
  le dernier tag, maintient une PR de release avec `CHANGELOG.md` +
  bump de version a jour, et cree le tag + GitHub Release quand cette PR est
  mergee. **La version bumpee est `workspace.package.version` dans le
  Cargo.toml racine** (extra-file TOML dans `release-please-config.json`),
  qui cascade aux 3 membres via `version.workspace = true` — pas de version
  par crate a maintenir separement.
- **`publish-release-assets.yml`** : dispatche par `release-please.yml` (pas
  par l'event `release: published` — GITHUB_TOKEN ne redeclenche pas
  d'autres workflows, meme raison que dans OuyouyouTube). Build release,
  zippe `unisense.exe` + `interception.dll` + `unisense-gui.exe` +
  `config/` + `vendor/` + README/LICENSE, attache au GitHub Release.
- **Important : les commits doivent suivre Conventional Commits** a partir
  de maintenant pour que le bump de version soit correct
  (`feat:` -> minor, `fix:` -> patch, `feat!:`/`BREAKING CHANGE:` -> major,
  `chore:`/`ci:`/`test:`/`build:` -> pas de bump, juste caches dans le
  changelog). Les commits d'avant l'ajout de ce pipeline ne suivaient pas
  cette convention — sans consequence, ils sont deja "dans" la base 0.1.0
  du manifest, release-please ne regarde que ce qui vient apres.

**Pas encore fait cote GitHub** : le depot n'a pas de remote pour l'instant
(voulu par l'utilisateur). Ces workflows ne tourneront qu'une fois pousses
sur un repo GitHub avec Actions active. Verifier a ce moment-la : Settings >
Actions > General > "Allow GitHub Actions to create and approve pull
requests" doit etre coche, sinon release-please ne peut pas ouvrir sa PR.

## Pas encore fait / pistes ouvertes

Voir section "Idees d'amelioration" du README (detection de processus au
premier plan, rechargement a chaud du YAML, overlay d'etat a l'ecran,
multi-souris, installeur, API locale, chaine de pointeurs dans la GUI,
selecteur de fichier natif). Rien de tout ca n'est commence.
