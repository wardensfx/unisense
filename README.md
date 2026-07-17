# unisense

Normalise la sensibilite de visee souris entre jeux : vous definissez **une
seule cible universelle** (cm de deplacement main pour 360°), et unisense
calcule et applique le facteur d'echelle qui la respecte dans chaque jeu
configure, quel que soit le DPI de la souris ou l'echelle de sensi interne du
jeu.

## Intention du projet

Aucun standard commun n'existe entre jeux pour la sensibilite de visee : un
"3" dans un jeu ne veut rien dire dans un autre, et retrouver "sa" sensi
d'un jeu a l'autre est une corvee manuelle sujette a erreur. unisense est un
outil de **confort et d'accessibilite** : il ne donne aucun avantage qu'un
joueur ne pourrait pas obtenir en calculant lui-meme le bon reglage a la
main (ce que font deja des sites comme mouse-sensitivity.com) ; il
automatise juste le calcul et son application. Aucune fonctionnalite de
triche (aimbot, ESP, macro de recul, etc.) n'est dans le perimetre du
projet et n'y sera jamais ajoutee.

## ⚠️ Avertissement important : anti-cheats a niveau noyau

unisense s'appuie sur le driver **Interception**, un driver noyau Windows.
De nombreux outils d'accessibilite/confort legitimes l'utilisent, mais
**certains anti-cheats a niveau noyau (Riot Vanguard pour Valorant, certains
modes de BattlEye/EAC) detectent et interdisent activement la presence de
drivers d'interception d'entree tiers**, meme sans intention de triche, et
peuvent entrainer un bannissement. Avant d'utiliser unisense sur un jeu
donne :

- Verifiez la politique de l'anti-cheat du jeu concernant les drivers
  d'entree tiers / les logiciels de remapping bas niveau.
- En cas de doute, **ne l'utilisez pas** sur ce jeu, ou testez d'abord sur un
  compte secondaire.
- Les auteurs de ce projet ne sont pas responsables des consequences
  (bannissement, etc.) de son usage sur un jeu dont l'anti-cheat l'interdit.

## Comment ca marche

1. Un thread dedie ouvre un contexte **Interception** et capture les rapports
   HID bruts de la souris, en amont de RawInput, des ballistics Windows et de
   toute mise a l'echelle DPI-aware du bureau.
2. Pour le jeu actif, unisense connait une constante publique
   (`deg_per_count_at_ref`, "degres de rotation camera par compte de souris
   brut, mesures a une sensibilite in-game de reference documentee").
3. A partir du DPI materiel declare, de cette constante, et de votre cible
   `target_cm_per_360`, unisense calcule un facteur multiplicatif.
4. Ce facteur est applique aux deltas X/Y bruts (multiplication lineaire
   simple, aucune courbe d'acceleration ajoutee), puis les deltas modifies
   sont reinjectes via Interception, indiscernables pour le jeu d'un
   mouvement natif de la souris.
5. Un hotkey (clavier ou bouton souris) bascule entre **passthrough**
   (facteur 1.0, pour les menus/le bureau) et **jeu actif**. La selection du
   jeu se fait via l'icone systray ou un raccourci clavier — pas de detection
   automatique de processus par defaut.

Le calcul complet est documente et teste dans `src/scaling.rs`.

## Installation

Le DLL utilisateur d'Interception (`interception.dll`) et l'installeur du
driver noyau sont **embarques dans ce depot** sous `vendor/interception/`
(binaires officiels non modifies, redistribues sous LGPL 3.0 — voir
`vendor/interception/NOTICE.md`) : pas besoin d'aller les telecharger sur
GitHub separement.

### 1. Driver Interception (obligatoire, etape systeme)

1. Ouvrez une invite de commande **en administrateur** dans
   `vendor/interception/` et lancez :
   ```
   install-interception.exe /install
   ```
2. **Redemarrez Windows** (le driver se charge au boot, l'installation seule
   ne suffit pas).

Pour desinstaller plus tard : `install-interception.exe /uninstall` puis
redemarrer.

### 2. Compiler unisense

Prerequis : [Rust](https://rustup.rs/) (edition 2021+), toolchain MSVC
(`rustup default stable-x86_64-pc-windows-msvc`). Le depot est un workspace
Cargo a trois membres : `core` (logique partagee), `app` (l'executable tray,
celui decrit ci-dessus) et `gui` (l'interface de configuration, voir plus
bas).

```
cargo build --release -p unisense
```

`app/build.rs` copie automatiquement `vendor/interception/interception.dll`
a cote de l'executable genere (`target/release/unisense.exe`) a chaque
build : rien a faire manuellement pour ce fichier. Copiez juste l'exe, le
DLL a cote (deja fait par le build), et le dossier `config/` la ou vous
voulez l'executer.

(`cargo build --release` sans `-p` compile aussi la GUI en plus — plus long,
et necessite le runtime WebView2, present par defaut sur Windows 11.)

> unisense doit generalement etre lance **en administrateur** : le driver
> Interception refuse `interception_create_context()` sinon.

### 3. Configuration

Copiez `config/games.example.yaml` vers `config/games.yaml` (a cote de
l'executable) et editez-le :

```yaml
settings:
  mouse_dpi: 800              # DPI actuel regle sur votre souris
  target_cm_per_360: 35.0     # votre cible universelle
  toggle_hotkey: "Mouse4"     # passthrough <-> dernier jeu actif
  cycle_game_hotkey: "Ctrl+Alt+F9"
  start_in_passthrough: true

games:
  - name: "Valorant"
    deg_per_count_at_ref: 0.07
    reference_sensitivity: 1.0
```

`unisense.exe chemin\vers\config.yaml` permet de surcharger le fichier de
config utilise (utile pour plusieurs profils).

### 4. Regler le jeu

Dans les options du jeu, mettez sa sensibilite in-game exactement sur la
valeur `reference_sensitivity` documentee (souvent `1.0`). C'est unisense,
et non le jeu, qui fait ensuite tout le travail d'echelle. Si le jeu ne
permet pas de saisir cette valeur exacte (slider a crans), renseignez la
valeur reellement utilisee dans `current_sensitivity` (voir commentaires
dans `config.rs` / le YAML) — **uniquement valable si la formule de
sensibilite du jeu est lineaire**, ce qui est vrai pour la plupart des
moteurs Source/Quake/Unreal/idTech mais faux pour certains jeux (Minecraft
par exemple a une courbe cubique).

## Interface graphique de configuration (optionnelle)

Editer `games.yaml` a la main fonctionne, mais un second executable,
`unisense-gui`, offre une interface pour le faire sans toucher au YAML :
ajout/suppression de jeux, chargement de la config d'exemple en un clic, et
un assistant pour l'`auto_detect` (5.2). Elle lit/ecrit exactement le meme
`config/games.yaml` que l'app tray (a cote de son propre executable — placez
les deux .exe dans le meme dossier).

```
cargo build --release -p unisense-gui
```

`target/release/unisense-gui.exe` — inutile de le lancer en administrateur
(sauf pour lire la memoire d'un jeu protege par un anti-cheat qui l'exige).

Fonctionnalites :

- Cadran de calibration : cible cm/360° et DPI, avec un apercu du facteur
  applique a chaque jeu de la liste (badge `×0.xx`) mis a jour en direct.
- Cartes de jeux : ajout, edition, suppression ; previsualisation du facteur
  pendant la saisie de la constante.
- **Charger l'exemple** : recharge le contenu de `games.example.yaml`
  (embarque dans le binaire au moment de la compilation, aucun acces
  reseau) comme point de depart.
- **Assistant de detection automatique** (dans l'editeur d'un jeu, section
  repliable) :
  - Un selecteur de processus (liste les process en cours) et de module,
    pour remplir `process_name`/`module_name` sans les taper a la main.
  - Un mini-scanner memoire "avant/apres" : vous capturez un instantane de
    la memoire du jeu dans un etat (ex. menu), un second dans l'autre etat
    (ex. en jeu), et l'outil liste les octets qui ont change entre les deux
    — un bouton "Utiliser" sur un candidat remplit directement
    `offset`/`in_game_bytes`. Lecture seule (`ReadProcessMemory`), plafonnee
    en volume scanne (~96 Mo de regions privees lisibles/inscriptibles) :
    c'est un scan "premiere passe" a la Cheat Engine, pas un outil de reverse
    engineering complet — a affiner en repetant l'operation si trop de
    candidats remontent.

## Hotkeys

Formats acceptes dans le YAML : `"F9"`, `"Ctrl+Alt+F9"`, `"Shift+F5"`,
`"Mouse3"` (clic molette), `"Mouse4"` (bouton arriere), `"Mouse5"` (bouton
avant). Les hotkeys clavier passent par l'API Windows standard
(`RegisterHotKey`, globales, fonctionnent meme jeu au premier plan) ; les
hotkeys souris sont detectees directement dans le flux Interception (les
boutons souris ne sont pas supportes par `RegisterHotKey`).

- `settings.toggle_hotkey` : bascule passthrough <-> dernier jeu utilise.
- `settings.cycle_game_hotkey` : passe au jeu suivant dans la liste (sort du
  passthrough si necessaire).
- `hotkey:` (optionnel, par jeu) : selectionne ce jeu directement.
- Clic gauche sur l'icone systray : equivalent a `toggle_hotkey`.
- Clic droit sur l'icone systray : menu (choix du jeu, passthrough, quitter).

## Calculer la constante d'un nouveau jeu

`deg_per_count_at_ref` = nombre de degres dont tourne la camera pour **un
seul compte** de mouvement souris brut, avec la sensibilite in-game fixee a
`reference_sensitivity`.

**Methode 1 — table publique** : cherchez le jeu sur
[mouse-sensitivity.com](https://www.mouse-sensitivity.com/) ou dans les
fichiers de config du jeu (ex : `m_yaw` pour les moteurs Source/GoldSrc,
souvent `0.022` par defaut — la constante est alors
`sensitivity_de_reference * m_yaw`).

**Methode 2 — mesure empirique** (fiable, independante des tables tierces) :

1. Fixez un DPI connu D (ex : 800) et la sensi in-game a votre reference
   (ex : 1.0).
2. Dans le jeu, tournez la camera d'exactement 360° en utilisant un tapis de
   souris avec repere (ou une regle + un point de reference visuel a
   l'ecran), et mesurez la distance parcourue en cm : `cm_360_mesure`.
3. Calculez :
   ```
   deg_per_count_at_ref = (2.54 * 360) / (cm_360_mesure * D)
   ```
   (c'est l'inverse de la formule utilisee par `scaling::compute_factor`,
   cf. commentaires en tete de `src/scaling.rs`).
4. Ajoutez l'entree dans `games.yaml` avec `reference_sensitivity` = la
   valeur utilisee a l'etape 1.

Revalidez la constante apres toute mise a jour majeure du jeu qui touche a
la sensibilite (patch notes a surveiller).

## Detection automatique (avancee, experimentale)

Le point 5.2 du cahier des charges (bascule automatique menu/gameplay par
lecture memoire) est implemente comme un **framework generique**, pas comme
des offsets pre-remplis pour des jeux precis : ce projet ne fournit et ne
maintient aucune adresse memoire, car elles sont specifiques a chaque
version d'un jeu et cassent au moindre patch.

Pour l'activer sur un jeu, ajoutez un bloc `auto_detect` a son entree (voir
l'exemple commente dans `games.example.yaml`) :

```yaml
auto_detect:
  process_name: "MonJeu.exe"
  module_name: "MonJeu.exe"   # optionnel, defaut = module principal
  offset: 0x00ABCDEF          # offset depuis la base du module
  pointer_chain: []           # chaine de pointeurs a suivre, si besoin
  in_game_bytes: [0x01]       # valeur attendue EN JEU (gameplay)
  poll_interval_ms: 250
```

Trouver `offset`/`in_game_bytes` demande de reverse-engineer le jeu (ex :
[Cheat Engine](https://www.cheatengine.org/), scan de valeur "01 en jeu / 00
au menu", puis "quel pointeur statique y mene"). C'est fait en lecture seule
(`ReadProcessMemory`), unisense n'ecrit jamais dans la memoire d'un autre
process. Voir aussi l'avertissement anti-cheat plus haut : lire la memoire
d'un process protege par un anti-cheat noyau peut aussi etre detecte/interdit
independamment d'Interception.

## Architecture du code

Workspace Cargo a trois membres :

```
core/                       unisense-core (lib, sans dependance Win32)
  src/config.rs              Schema + chargement/sauvegarde du YAML
  src/scaling.rs              Calcul du facteur + accumulateur (teste)

app/                        unisense (l'executable tray, decrit plus haut)
  build.rs                   Copie vendor/interception/interception.dll
  src/interception.rs         Bindings FFI vers interception.dll
  src/hotkey.rs                Parsing "Ctrl+Alt+F9" / "Mouse4"
  src/capture.rs                Thread de capture (scaling + hotkeys souris)
  src/state.rs                   Etat partage (mode, facteurs, accumulateurs)
  src/tray.rs                     Fenetre Win32 invisible, systray, hotkeys
  src/memory_watch.rs              Detection auto (5.2), framework generique
  src/main.rs                       Cablage de tout ce qui precede

gui/                        unisense-gui (interface de configuration)
  src-tauri/build.rs          tauri_build::build()
  src-tauri/src/sysinfo.rs     Process/module/scan memoire (Win32)
  src-tauri/src/commands.rs     Commandes exposees au frontend
  src-tauri/src/main.rs          Cablage Tauri
  frontend/                       HTML/CSS/JS statique (pas de bundler)
```

`core` est partage entre `app` et `gui` pour que le calcul du facteur et le
schema de config restent une seule source de verite.

Un seul facteur lineaire est applique (`x' = x * F`, `y' = y * F`) : pas de
courbe d'acceleration, de smoothing ni de capping ajoutes par l'outil, comme
demande.

## Limitations connues

- Windows/Interception ne peuvent pas lire le DPI materiel de la souris :
  `mouse_dpi` doit etre tenu a jour manuellement dans la config si vous le
  changez sur la souris.
- La correction `current_sensitivity`/`reference_sensitivity` suppose une
  formule de sensibilite lineaire cote jeu ; faux pour quelques jeux
  (courbes non lineaires), voir plus haut.
- `deg_per_count_at_ref` peut devenir obsolete apres une mise a jour du jeu.
- Pas de detection automatique de processus pour le changement de jeu par
  defaut (choix delibere du cahier des charges) ; combinable avec
  `auto_detect` (5.2) si vous etes pret a maintenir vos propres offsets.

## Idees d'amelioration

- **Detection de processus au premier plan** (`GetForegroundWindow` +
  correspondance nom d'exe -> jeu configure) comme alternative plus fiable
  et moins fragile que la lecture memoire pour au moins savoir *quel jeu*
  est actif (le distinguo menu/gameplay resterait manuel ou via
  `auto_detect`).
- **Rechargement a chaud** du YAML (watcher de fichier) sans relancer
  l'appli tray.
- **Indicateur a l'ecran** (overlay discret) du mode actif, pour ceux qui ne
  regardent pas la zone de notification.
- **Multi-souris** : le code gere deja des accumulateurs par
  `InterceptionDevice`, mais il n'y a pas encore de moyen de configurer un
  DPI different par souris physique.
- **Signature/installeur** (MSI ou script d'installation guidee du driver +
  de l'appli + creation de la tache de demarrage automatique).
- **API locale (named pipe)** pour piloter le changement de jeu/mode depuis
  un Stream Deck, un script AutoHotkey, ou un launcher de jeu tiers.
- **Chaine de pointeurs dans la GUI** : `pointer_chain` (utile pour les
  adresses qui bougent a chaque lancement du jeu) n'est editable qu'a la
  main dans le YAML pour l'instant, pas depuis l'assistant de detection.
- **Selecteur de fichier natif** dans la GUI pour choisir un `games.yaml`
  ailleurs que dans le dossier conventionnel a cote de l'exe (actuellement
  pas de dependance a un plugin de dialogue Tauri, pour rester minimal).

## Contribuer / versionnage

Le numero de version et le `CHANGELOG.md` sont geres automatiquement par
[release-please](https://github.com/googleapis/release-please) a partir des
messages de commit sur `main`, au format
[Conventional Commits](https://www.conventionalcommits.org/) :

- `feat: ...` -> version mineure
- `fix: ...` -> version corrective
- `feat!: ...` / pied `BREAKING CHANGE: ...` -> version majeure
- `chore:`, `ci:`, `docs:`, `test:`, `build:`, `refactor:`, `perf:` -> pas de
  bump de version (mais entree de changelog pour `docs`/`perf`/`refactor`)

A chaque release, `.github/workflows/publish-release-assets.yml` compile et
attache un zip (`unisense.exe` + `interception.dll` + `unisense-gui.exe` +
`config/` + `vendor/`) au GitHub Release. Voir `CLAUDE.md` pour le detail du
pipeline.

## Licence

MIT, voir `LICENSE`, pour le code source d'unisense (`core/`, `app/`,
`gui/src-tauri/`, `gui/frontend/*.{html,css,js}`).

Composants tiers redistribues (binaires non modifies, licences separees —
voir chaque NOTICE) :
- `vendor/interception/` : Interception (LGPL 3.0, usage non commercial —
  voir `vendor/interception/NOTICE.md`).
- `gui/frontend/fonts/` : Space Grotesk (SIL OFL 1.1 — voir
  `gui/frontend/fonts/NOTICE.md`).
