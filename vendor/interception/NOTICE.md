# Provenance et licence des fichiers de ce dossier

Ces fichiers sont redistribues tels quels depuis la release officielle
`v1.0.1` du projet **Interception** :

- Depot : https://github.com/oblitum/Interception
- Release : https://github.com/oblitum/Interception/releases/tag/v1.0.1
- Fichier source : `library/x64/interception.dll` (et `command line
  installer/install-interception.exe`) de l'archive `Interception.zip`
  publiee a cette release.

Aucun de ces binaires n'a ete modifie.

## Licence

Interception est en double licence :

- **Usage commercial** : licence separee, voir
  `licenses/commercial-usage/` dans le depot amont (non applicable ici).
- **Usage non commercial** : **LGPL 3.0**, voir `licenses/non-commercial-usage/LGPL 3.0.txt`
  dans le depot amont (texte integral reproduit dans `LGPL 3.0.txt` a cote
  de ce fichier).

unisense est un outil gratuit et non commercial (licence MIT, voir
`LICENSE` a la racine du depot) : c'est donc la voie **LGPL 3.0** qui
s'applique a la redistribution de `interception.dll` faite ici.

`interception.dll` reste un composant distinct sous LGPL 3.0, separe du
code source d'unisense (MIT). Conformement a la LGPL :

- Le code source correspondant est publiquement et durablement disponible
  a l'adresse ci-dessus (depot amont officiel, release v1.0.1).
- unisense charge `interception.dll` **dynamiquement au runtime**
  (`libloading`, pas de lien statique / pas de `interception.lib` embarque
  dans le binaire d'unisense) : un utilisateur peut donc remplacer ce
  fichier par sa propre version (recompilee ou modifiee) de la
  bibliotheque sans avoir a recompiler unisense, ce qui satisfait
  l'exigence de "remplacement de la bibliotheque" de la LGPL.

Le driver noyau (`install-interception.exe`, qui installe le `.sys`) est
inclus ici uniquement par commodite d'installation ; voir le README a la
racine du depot pour la procedure.
