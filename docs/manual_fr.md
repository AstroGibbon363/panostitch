# Panorama Stitcher : manuel d'utilisation

*Autres langues : [English](manual_en.md) · [Česky](manual_cs.md) · [Deutsch](manual_de.md) · [Español](manual_es.md) · [Polski](manual_pl.md)*

Un programme de bureau qui assemble des photos qui se chevauchent en un
panorama ou une mosaïque : images JPEG, TIFF, PNG, RAW et FITS astronomiques.
Chaque étape ne s'exécute que lorsque vous appuyez sur son bouton, et vous
pouvez vérifier et corriger chaque résultat avant de continuer. D'autres
onglets empilent des images d'étoiles en un ciel nocturne peu bruité ou en
filés d'étoiles ([Empilement d'étoiles](#13-empilement-détoiles)), finissent les images
([Post-traitement](#16-post-traitement)) et font des vidéos ([Timelapse](#17-timelapse)).

Les commandes figurent dans ce manuel **en gras**, telles que vous les voyez à
l'écran en français. La langue de la fenêtre se change en haut à droite ; ce
manuel se trouve aussi dans le programme, sous **? Aide** (F1).

---

## Sommaire

1. [Configuration requise](#1-configuration-requise)
2. [Lancer le programme](#2-lancer-le-programme)
3. [La fenêtre en un coup d'œil](#3-la-fenêtre-en-un-coup-dœil)
4. [Démarrage rapide](#4-démarrage-rapide)
5. [Pas à pas](#5-pas-à-pas)
6. [Se déplacer dans la vue](#6-se-déplacer-dans-la-vue)
7. [Projections](#7-projections)
8. [Fichiers RAW](#8-fichiers-raw)
9. [Fichiers FITS (astronomie)](#9-fichiers-fits-astronomie)
10. [Enregistrer](#10-enregistrer)
11. [Dépannage](#11-dépannage)
12. [Conseils pour de bons panoramas](#12-conseils-pour-de-bons-panoramas)
13. [Empilement d'étoiles](#13-empilement-détoiles)
14. [Utilisation sans la fenêtre](#14-utilisation-sans-la-fenêtre)
15. [Fichiers](#15-fichiers)
16. [Post-traitement](#16-post-traitement)
17. [Timelapse](#17-timelapse)

---

## 1. Configuration requise

- **Rien à installer.** Le programme est un seul fichier, `panostitch`
  (`panostitch.exe` sous Windows), plus `panostack` pour empiler des étoiles en
  ligne de commande. Sous Linux, il faut un pilote graphique avec OpenGL et X11
  ou Wayland, comme sur tout bureau.
- **Les fichiers RAW** ne demandent rien de plus : un décodeur RAW est intégré.
  `dcraw_emu` de LibRaw, RawTherapee, darktable ou dcraw sont aussi utilisés
  s'ils sont installés (voir [Fichiers RAW](#8-fichiers-raw)).
- **Les fichiers FITS et XISF** ne demandent rien de plus.
- **Les boîtes de dialogue de fichiers** ne demandent rien sous Windows et
  macOS. Sous Linux, elles utilisent `zenity`, `kdialog` ou `yad`, selon ce qui
  est installé (la plupart des bureaux en ont un). Sans eux, tapez les chemins
  dans le champ de chemin.

Pour compiler le programme à partir du code source, il faut Rust
(<https://rustup.rs>) : `cargo build --release` produit
`target/release/panostitch` et `target/release/panostack`.

---

## 2. Lancer le programme

Double-cliquez sur `panostitch`, ou lancez-le depuis un terminal :

```
panostitch
```

Vous pouvez aussi indiquer des photos ou un fichier de projet à ouvrir tout de
suite :

```
panostitch ~/Images/pano/*.NEF
panostitch mon_pano.panoproj
panostitch nuit.stackproj           # s'ouvre dans l'onglet Empilement d'étoiles
panostitch nuit.tlproj              # s'ouvre dans l'onglet Timelapse
```

Remarques :

- Le programme utilise automatiquement les cœurs du processeur. Les longues
  tâches en utilisent au plus 85 % et laissent 15 % de la mémoire libres, pour
  que l'ordinateur reste utilisable.
- La fenêtre s'ouvre en une ou deux secondes ; rien n'est à préparer.
- Si aucune fenêtre n'apparaît, lancez le programme depuis un terminal : les
  messages d'erreur s'y affichent.
- Les interrupteurs d'apparence (**Sombre**, **Moderne**, **Simple**) sont gardés
  dans `ui.toml` dans le dossier de réglages : `~/.config/panostitch` sous
  Linux, `~/Library/Application Support/panostitch` sous macOS,
  `%APPDATA%\panostitch` sous Windows.
- **Langue :** la fenêtre parle français, anglais, tchèque, allemand, espagnol
  ou polonais ; le menu en haut à droite change aussitôt (le premier
  démarrage suit la langue du système).
- **Aide :** **? Aide** en haut à droite (ou F1) ouvre ce manuel dans le
  programme, avec sommaire, recherche et liens ; F1 ouvre le chapitre de
  l'onglet affiché.
- **Mémorisé d'une session à l'autre :** l'onglet ouvert, les réglages
  d'empilement, les réglages vidéo du timelapse (anti-scintillement, mouvement
  de caméra, sortie, **RAW via**) et les réglages RAW de l'assemblage. Les
  fichiers, masques et rendus ne sont pas mémorisés ; un fichier de projet les
  conserve.
- **Glisser-déposer :** déposez des images ou un dossier sur la fenêtre pour les
  ajouter à l'onglet affiché (Post-traitement ouvre la première image). Un
  fichier de projet déposé s'ouvre dans son propre onglet.
- **Projets récents** (barre du haut) liste les derniers projets ouverts ou
  enregistrés, de tout type ; un clic en ouvre un dans son onglet.
- Les boîtes de dialogue de fichiers s'ouvrent dans le dernier dossier
  utilisé.
- Après l'enregistrement, **Afficher dans le dossier** (dans le rapport, ou à
  côté de **⬇ Enregistrer** dans Post-traitement) ouvre le dossier dans le
  gestionnaire de fichiers.

---

## 3. La fenêtre en un coup d'œil

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ [Open…] [Save proj.] │ [Image+keypoints] [Pair matches]     │ 3 · Projection      │
│ Images               │ [Panorama] [Zoom +] [Zoom -] [Fit]   │  projection menu    │
│  Add Remove Clear ▲▼ │ (o) overlays  [Undo] [Redo]          │  Focal, Yaw, Pitch, │
│  path box            │                                      │  Roll               │
│  image list          │                                      │ 4 · Compose         │
│  Reference           │            main view                 │  Blend, Preview size│
│  RAW via, half size  │                                      │  Exposure,Brightness│
│  Preset              │                                      │  show unlinked      │
│ 1 · Features         │                                      │  Auto crop, Uncrop  │
│                      │                                      │  Nudge ← ↑ ↓ →      │
│ 2 · Matching         │                                      │ 5 · Stretch         │
│  Pairs, Model, Pair  │                                      │  histogram, sliders │
│  pair details        │  status line (red text)              │ [Preview] [Save…]   │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

(Le schéma montre les noms en anglais ; dans la fenêtre, ils sont dans la
langue choisie.)

- **Colonne de gauche :** ouvrir et enregistrer des projets, choisir les
  images, trouver les points clés, apparier les images.
- **Centre :** la vue principale, avec des boutons pour changer de vue,
  zoomer, masquer les superpositions et annuler ou rétablir.
- **Colonne de droite :** projection, exposition, fusion, luminosité,
  placement manuel, étirement de l'histogramme, aperçu et enregistrement.
- **Ligne d'état rouge** (sous la vue) : ce que fait le programme, les
  résultats et les messages d'erreur.

Les interrupteurs en haut à droite changent l'apparence : **Sombre**,
**Moderne** (commandes plus arrondies et aérées) et **Simple**. Le mode simple
masque les réglages détaillés (détection et appariement, fusion, étirement,
décalage fin) et garde ce dont la plupart des panoramas ont besoin : les
images, un préréglage, **Exécuter toutes les étapes**, la projection avec
focale, lacet, tangage et roulis, l'exposition, **Égaliser**, le recadrage,
l'aperçu et l'enregistrement. Les réglages masqués gardent les valeurs du
préréglage. Le programme mémorise les interrupteurs.

---

## 4. Démarrage rapide

**Panorama photo :**

1. Laissez **RAW en demi-taille** activé. C'est plus rapide et suffisant pour
   vérifier le résultat.
2. Choisissez un **Préréglage** adapté au sujet (voir [Préréglages](#50-préréglages)).
3. Tapez le dossier de vos photos dans le champ de chemin et appuyez sur
   **Entrée**, ou cliquez sur **Ajouter…** et sélectionnez les photos. **L'ordre
   n'a pas d'importance.**
4. Cliquez sur **Exécuter toutes les étapes** et attendez que la ligne d'état
   indique *Terminé*.
5. Choisissez une **projection** à droite : **cylindrique** pour une seule
   rangée, **sphérique** pour plusieurs rangées ou ce qui monte ou descend très
   loin. Cliquez sur **Exposition automatique** si c'est trop sombre, et
   ajustez **Tangage (°)** si l'horizon est courbé. Cliquez sur **Recadrage
   automatique** pour couper les bords irréguliers.
6. Cliquez sur **⬇ Enregistrer le panorama…**. Cliquez sur **⬇ Enregistrer le
   projet…** si vous voulez y revenir plus tard.

**Mosaïque de ciel profond à partir de fichiers FITS :**

1. Choisissez le préréglage **mosaïque de ciel profond**.
2. Chargez les tuiles FITS (empilées ou images seules). Elles sont étirées
   automatiquement pour l'affichage.
3. Cliquez sur **Exécuter toutes les étapes**, puis sur **Étirement
   automatique** pour adapter l'étirement à toute la mosaïque, et retouchez-le
   si vous voulez.
4. Enregistrez en **`.fits`** pour garder les données linéaires pour la suite
   du traitement, ou en **`.tif`** pour obtenir l'image étirée que vous voyez.

---

## 5. Pas à pas

**Saisir des valeurs :** chaque curseur a un champ numérique à côté. Cliquez
dans le champ, tapez une valeur et appuyez sur **Entrée**. La valeur s'aligne
sur le pas et la plage du curseur.

### 5.0 Préréglages

Le menu **Préréglage** (colonne de gauche, sous **RAW via**) règle d'un coup tous
les paramètres d'appariement et de composition pour un type de sujet. Chaque
valeur peut encore être changée ensuite.

| Préréglage | Pour | Réglages principaux |
|---|---|---|
| **paysage de jour** | panoramas de jour, appareil tourné sur place | débruitage léger, homographie, cylindrique |
| **astro / paysage de nuit** | ciels nocturnes, Voie lactée sur un paysage | fort débruitage, appariement plus souple, sphérique, +1 EV (puis **Exposition automatique**), seuil des coins plus haut (moins de faux coins dans le bruit) |
| **mosaïque de ciel profond** | mosaïques de champs stellaires au télescope, p. ex. FITS | peu de débruitage (les étoiles sont minuscules), modèle de similitude (décalage + rotation de champ), pas d'affinage, rectilinéaire ; utilisez **Étirement automatique** |
| **macro / microscope** | une lame ou une platine déplacée sous un appareil fixe | modèle de translation (les images ne font que se décaler), pas d'affinage, rectilinéaire |
| **scan / document plat** | cartes, tableaux, documents photographiés par morceaux | modèle affine, pas d'affinage, rectilinéaire |

### 5.1 Images

| Commande | Effet |
|---|---|
| **Ajouter…** | Ouvre une boîte de dialogue de fichiers. Vous pouvez choisir plusieurs fichiers (Ctrl/Maj-clic). |
| **Champ de chemin** | Tapez le chemin d'un fichier ou d'un **dossier** et appuyez sur Entrée. Un dossier charge toutes ses images, triées par nom. Les fichiers nommés `panorama…` (résultats précédents) sont ignorés. |
| **Liste des images** | Cliquez sur une image pour la sélectionner et l'afficher. **Ctrl-clic** ajoute ou retire une image de la sélection, **Maj-clic** sélectionne une plage et **Ctrl+A** (souris sur la liste) sélectionne tout. Faites défiler avec la molette. |
| **Retirer** | Retire les images sélectionnées. La touche **Suppr** fait de même quand la souris est sur la liste. |
| **Tout vider** | Retire toutes les images. |
| **▲ / ▼** | Monte ou descend les images sélectionnées dans la liste. Seuls les modes de paires *séquentiel* et *grille* tiennent compte de l'ordre. |
| **Référence** | L'image autour de laquelle les autres sont placées ; elle reste non déformée. **auto (centre)** prend l'image au milieu du groupe, ce qui donne en général le résultat le plus homogène. |
| **RAW via** | Le convertisseur RAW à utiliser ; voir [Fichiers RAW](#8-fichiers-raw). S'applique aux images ajoutées ensuite. |
| **RAW en demi-taille** | Décode les RAW et les FITS de caméras couleur en demi-résolution : environ 4× plus rapide et ¼ de la mémoire. S'applique aux images ajoutées ensuite. Les RAW sont relus en taille réelle quand vous enregistrez avec une **Taille de sortie** supérieure à 50 %. |

La liste marque chaque entrée :

| Marque | Signification |
|---|---|
| ligne bleue | sélectionnée ; la plus foncée est l'image courante (pour l'affichage, la luminosité et le déplacement) |
| `R` | image de référence |
| `4345kp` | nombre de points clés trouvés |
| `UNLINKED` | l'image n'a pas pu être reliée au panorama (voir [5.3](#53-étape-2--appariement)) |

Les résultats d'appariement sont conservés quand vous réordonnez des images ou
en retirez d'autres, il n'est donc pas nécessaire d'apparier à nouveau.

### 5.2 Étape 1 : points clés

Les points clés sont des endroits caractéristiques, comme des coins, des bords
et des étoiles, reconnaissables dans deux images qui se chevauchent.

| Curseur | Par défaut | Signification |
|---|---|---|
| **Taille de détection (px)** | 2000 | L'image est réduite à cette taille (plus grand côté) avant la recherche. Plus grand est plus précis mais plus lent. |
| **Points clés max.** | 5000 | Limite par image. Les points sont répartis sur toute l'image. |
| **Seuil des coins** | 0.001 | Force minimale d'un coin. Plus bas trouve des coins plus faibles (utile pour le ciel ou le brouillard) ; plus haut ne garde que les forts. |
| **Débruitage (σ px)** | 1.5 | Floute le bruit du capteur avant la recherche. Montez-le (2–3) pour les **photos de nuit** ou à ISO élevé bruitées ; baissez-le (0,5–1) pour les champs d'étoiles et les photos de jour propres et détaillées. |

Cliquez sur **Détecter les points clés**. La vue montre l'image courante avec
les points en **cercles jaunes**. Ils doivent être répartis sur les parties de
l'image qui chevauchent d'autres images. Seules les images dont les réglages
ont changé sont retraitées. Les images sombres et linéaires (photos de nuit,
FITS) sont prises en charge : la détection ajuste le contraste en interne.

### 5.3 Étape 2 : appariement

L'appariement retrouve les mêmes points clés dans deux images. Ensuite,
**RANSAC** ne garde que les correspondances cohérentes avec un seul modèle
géométrique (les *inliers*).

**Quelles paires sont essayées (menu Paires) :**

| Paires | À utiliser quand |
|---|---|
| **toutes les paires** | Par défaut. Chaque image est essayée avec toutes les autres, donc **l'ordre n'a pas d'importance** et plusieurs rangées fonctionnent seules. Plus long avec beaucoup d'images (74 photos ≈ 35 s). |
| **séquentiel** | Une rangée, images dans l'ordre de prise de vue. Chaque image n'est appariée qu'avec la suivante. Le plus rapide. |
| **grille** | Plusieurs rangées dans un ordre connu : **Colonnes de la grille** images par rangée, rangée après rangée. Les voisines à droite, en dessous et en diagonale sont essayées. Activez **serpentin** si une rangée sur deux a été prise en sens inverse. |

**Réglages :**

| Commande | Par défaut | Signification |
|---|---|---|
| **Colonnes de la grille** | 5 | Images par rangée (mode grille seulement). |
| **Test du ratio** | 0.8 | De combien la meilleure correspondance doit dépasser la deuxième. Plus bas est plus strict (moins de correspondances, plus fiables) ; plus haut en donne davantage. |
| **Distance max.** | 0.35 | À quel point deux points clés peuvent différer tout en correspondant. |
| **Tol. RANSAC (px)** | 5.0 | À quelle distance (en pixels) une correspondance peut être du modèle tout en comptant comme inlier. |
| **Inliers min.** | 20 | Nombre minimal d'inliers pour qu'une paire compte comme un vrai recouvrement. |
| **Modèle** | homographie | Comment une image se projette sur une autre, voir plus bas. |
| **affiner** | activé | Affinage global, voir plus bas. |

**Quand une paire compte comme un vrai recouvrement :** il lui faut au moins
*Inliers min.* inliers, une nette majorité de ses correspondances doit
concorder et la transformation doit être plausible (elle ne doit pas replier
ni étirer exagérément l'image). Cela élimine les fausses correspondances entre
images qui ne se chevauchent pas.

**Modèles :**

| Modèle | À utiliser quand |
|---|---|
| **homographie** | Panoramas normaux : appareil tourné sur place. Par défaut. |
| **affine** | Scans, ou photos d'un objet plat pris de loin. |
| **similitude** | Seulement décalage, rotation et mise à l'échelle uniforme, p. ex. mosaïques au télescope avec rotation de champ. Stable avec peu de correspondances. |
| **translation** | Seulement décalage, p. ex. un scanner ou une platine de microscope. Le plus robuste, le moins souple. |

**Placement et affinage global (affiner) :** les images sont reliées par leurs
paires les plus fortes, à partir de l'image de référence. Avec **affiner**
activé (modèle homographie seulement), toutes les paires sont ensuite résolues
ensemble comme un seul appareil qui tourne. Les petites erreurs se répartissent
au lieu de s'additionner, et les boucles entre rangées se referment ; cela
supprime la plupart des fantômes. L'affinage estime aussi la distorsion en
barillet de l'objectif (forte sur les grands-angles) et la corrige au rendu.
Les paires qui ne cadrent pas avec la solution commune sont **rejetées
automatiquement**, car ce sont en général de fausses correspondances ou
quelque chose qui a bougé. La ligne d'état indique combien ont été rejetées et
l'**erreur typique par paire** (1–3 px, c'est bien).

**Ciels de nuit : appariement par les étoiles.** Les points de coin apparient
rarement des images qui ne montrent que du ciel : chaque étoile ressemble à
toutes les autres. Quand les points d'une paire échouent, ce sont donc ses
**étoiles** qui sont comparées : des triangles d'étoiles voisines, par les
angles entre elles. Ces angles ne changent ni quand l'appareil tourne ni quand
le ciel tourne, si bien que des images prises à quelques minutes d'écart
s'apparient quand même. Ces paires portent *(étoiles)* dans le menu **Paire**
et la ligne d'état les compte. Seules les paires ayant assez d'étoiles dans le
recouvrement, dont une bonne part correspond, sont acceptées. Quand les
appareils visent très loin l'un de l'autre (une rangée vers le haut, une vers
le bas), les coins éloignés d'une photo sont derrière l'autre appareil ; seule
la partie couverte par les correspondances est alors jugée.

**Un zoom qui a glissé.** Tourné vers le haut ou le bas, une bague de zoom peut
tourner sous son propre poids entre les prises. Avec des fichiers RAW, la
focale de chaque photo est connue : les points sont cherchés à la même échelle
dans toutes, et l'affinage ajuste chaque focale séparément. La ligne d'état
indique alors *Le zoom a changé entre les prises (15–17 mm)*.

**Le ciel qui tourne.** Le ciel tourne de 15° par heure : des rangées prises à
dix minutes d'écart le voient décalé de 2,5°. Avec des fichiers RAW (ils
enregistrent l'heure de chaque prise), l'affinage mesure cette rotation à
partir des paires d'étoiles et des paires au sol ensemble, et les appareils
sont bien placés. La ligne d'état indique *le ciel a tourné de 14,9°/heure
entre les prises* : proche de 15, c'est réussi. Au rendu, les étoiles de toutes
les photos apparaissent comme à un même instant (le milieu de la séance), tandis
que le sol et la lueur du ciel restent à leur place ; près des arbres et autres
silhouettes, chaque photo reste telle que prise.

**Boutons :**

- **Apparier** : apparie les paires choisies dans le menu Paires (en
  parallèle), puis place et affine.
- **Réapparier la paire** : apparie seulement la paire affichée, avec les
  réglages actuels.
- **Écarter la paire** : n'utilise plus la paire affichée (pour une
  correspondance visiblement fausse) ; le placement est refait sans elle.
- **Exécuter toutes les étapes** : points clés, appariement, placement et
  aperçu d'un coup.

La vue **Correspondances de la paire** montre deux images côte à côte :

- **lignes vertes** : inliers (correspondances correctes)
- **lignes rouges** : aberrants. Masquez-les avec l'interrupteur **aberrants**.

Le menu **Paire** liste les paires acceptées avec leur nombre d'inliers. Les
paires marquées *rejetée* ont échoué à un contrôle ; le texte sous le menu dit
pourquoi.

**À quoi ressemble un bon résultat :** des dizaines de lignes vertes ou plus,
décrivant toutes le même décalage, et une erreur moyenne de 1–2 px.

**Images non reliées :** une image sans paire acceptée avec les autres est
marquée `UNLINKED` dans la liste et **laissée hors du panorama**, pour ne pas
le gâcher. Activez **montrer les images non reliées** (colonne de droite) pour
afficher ces images sur une rangée sous le panorama et les placer à la main
avec **Ctrl+glisser**.

### 5.4 Étape 3 : projection

Voir [Projections](#7-projections) pour l'aspect de chacune.

| Commande | Signification |
|---|---|
| **Menu de projection** | 14 projections, voir [Projections](#7-projections) |
| **Focale (× auto)** | La focale est estimée à partir des correspondances. Ce curseur la met à l'échelle. Si le panorama semble comprimé ou étiré en largeur, essayez des valeurs entre 0,8 et 1,2. |
| **Lacet (°)** | Tourne tout le panorama vers la gauche ou la droite : choisit quelle direction est au centre. Utile pour les panoramas à 360° (p. ex. pour mettre le cœur de la Voie lactée au milieu plutôt qu'au bord). |
| **Tangage (°)** | Incline tout le panorama vers le haut ou le bas. Servez-vous-en pour **redresser un horizon courbé** (par exemple si l'appareil visait vers le bas). |
| **Roulis (°)** | Fait pivoter tout le panorama. Servez-vous-en pour mettre de niveau un horizon penché. |
| *Réglage fin* | Lacet, tangage et roulis ont chacun un curseur, un champ numérique et **◀ ▶**. Les curseurs de tangage et de roulis sont fins près de 0° (là où sont en général les corrections) et grossiers plus loin. Faites glisser lentement le champ numérique pour 0,02° par pixel, ou double-cliquez et tapez. **◀ ▶** avancent de 0,1° (avec Maj, de 1°). |
| **Réinitialiser focale / lacet / tangage / roulis** | Remet ces quatre curseurs à leurs valeurs par défaut. |

Le titre de la vue indique la taille du panorama, la focale en pixels, le
champ de l'image de référence et l'erreur d'alignement typique.

Ces commandes mettent l'aperçu à jour aussitôt (dès qu'un aperçu existe).

### 5.5 Étape 4 : composition

| Commande | Signification |
|---|---|
| **Fusion** | **intelligent** (par défaut) : fusion multibande avec suppression des fantômes. Le détail fin vient d'une seule image à chaque endroit, il n'y a donc pas d'images doubles de personnes, de lumières ou d'objets proches. Les différences de luminosité sont fondues sur une large zone, il n'y a donc pas de raccords durs. Là où les images qui se chevauchent ne concordent pas (quelque chose a bougé), toute la zone vient d'une seule image. **fondu** : mélange doux simple ; montre des fantômes là où quelque chose a bougé. **couture** : chaque pixel vient de l'image la plus centrale ; pas de fantômes, mais des raccords durs. **moyenne** : simple moyenne ; surtout pour vérifier l'alignement. |
| **Taille de l'aperçu (px)** | Taille de l'aperçu (plus grand côté). Plus grand est plus net mais plus lent. Zoomer calcule de toute façon le détail (voir [Se déplacer dans la vue](#6-se-déplacer-dans-la-vue)). |
| **Exposition (EV)** | Éclaircit ou assombrit tout le panorama, de −3 à +8 EV (+1 EV = deux fois plus lumineux). Les panoramas astro sombres demandent souvent +1,5 à +4. |
| **Luminosité (sél.)** | Facteur de luminosité des images **sélectionnées**. |
| **montrer les images non reliées** | Montre aussi les images qui n'ont pas pu être reliées, sur une rangée sous le panorama, pour les placer à la main. |
| **Exposition automatique** | Règle l'**Exposition** d'après l'aperçu actuel : la luminosité typique en gris moyen, sans écrêter plus que les 0,5 % les plus brillants (étoiles, lampes). |
| **Égaliser** | Calcule des facteurs de luminosité par image pour que les recouvrements concordent. |
| **Réinitialiser les décalages** | Annule tous les déplacements manuels. |
| **Recadrage automatique** | Coupe le panorama au plus grand rectangle entièrement couvert d'images, les bords noirs et irréguliers disparaissent. L'aperçu montre alors le résultat recadré, et **⬇ Enregistrer le panorama…** l'enregistre. La ligne d'état indique quelle part est gardée. Redressez d'abord l'horizon (tangage, roulis) ; un panorama courbé ne laisse qu'un rectangle étroit. |
| **Annuler le recadrage** | Retire le recadrage. Il est aussi retiré automatiquement quand vous changez quoi que ce soit qui déplace les images (projection, focale, lacet, tangage, roulis, déplacements, appariement). |
| **✂ Recadrer** (au-dessus de la vue) | Délimitez la partie du panorama à garder : tracez un cadre, ajustez-le par ses bords ou ses coins, faites-le glisser de l'intérieur pour le déplacer, puis **✂ Recadrage : terminé** (Échap annule). Seule cette partie est calculée et enregistrée : un grand panorama demande moins de mémoire, et l'onglet Post-traitement n'est pas nécessaire pour un simple recadrage. Il s'annule comme le recadrage automatique et disparaît quand la géométrie change. |
| **⬇ Enregistrer et post-traiter…** (au-dessus de la vue) | Enregistre le panorama, puis l'ouvre dans l'onglet Post-traitement. |
| **Décaler la sélection ← ↑ ↓ →** | Déplace l'image courante d'un pixel d'aperçu (voir plus bas). |

Dans la vue **Panorama**, le contour de chaque image est dessiné et numéroté :

- **orange** : l'image courante
- **bleu clair** : les autres images sélectionnées
- **cyan** : l'image de référence
- **blanc** : les autres images
- **lignes jaunes en pointillés** : les liens utilisés pour placer les images

Désactivez-les avec **superpositions** dans la barre d'outils pour voir le
panorama seul.

**Cliquez dans un contour** pour sélectionner cette image (fonctionne aussi
sans superpositions).

**Déplacer des images à la main :** maintenez **Ctrl** et faites glisser une
image, ou utilisez les flèches du clavier ou les boutons de décalage. L'image
bouge avec les images qui y sont rattachées plus loin de la référence (le long
des liens jaunes). L'image de référence ne bouge pas ; utilisez le tangage et
le roulis pour tout déplacer.

### 5.6 Étape 5 : étirement

Les images astronomiques (et les rendus RAW sombres) sont en général
**linéaires** : le détail intéressant se trouve dans les quelques pour cent
les plus sombres de la plage de luminosité et l'image paraît presque noire.
L'étirement de l'histogramme le rend visible.

| Commande | Signification |
|---|---|
| **Histogramme** | Répartition de la luminosité de l'aperçu du panorama (dans la vue Panorama) ou de l'image courante (dans les autres vues), en échelle logarithmique. La **ligne bleue** marque le point noir, la **ligne blanche** le point blanc, et la **courbe orange** montre comment la luminosité d'entrée (de gauche à droite) devient la luminosité de sortie (de bas en haut). |
| **Point noir** | Tout ce qui est à cette luminosité ou en dessous devient noir. Placez-le juste sous le fond du ciel (le bord gauche du pic principal de l'histogramme). |
| **Tons moyens** | Éclaircit (valeurs plus petites) ou assombrit (valeurs plus grandes) les tons moyens sans écrêter. 0,5 est neutre. Les images de ciel profond demandent en général 0,005–0,05. |
| **Point blanc** | Tout ce qui est à cette luminosité ou au-dessus devient blanc. |
| **Étirement automatique** | Règle les trois d'après l'histogramme actuel : point noir juste sous le fond du ciel, tons moyens pour que le fond arrive vers 25 % de gris (la même idée que l'étirement d'écran automatique de PixInsight). |
| **Réinitialiser l'étirement** | Retour à aucun étirement (0 / 0,5 / 1). |

- L'étirement ne change que la façon dont les images sont **affichées** et
  dont le panorama est enregistré en TIFF, PNG ou JPEG. L'appariement travaille
  toujours sur les données d'origine.
- **Les fichiers FITS** reçoivent un étirement automatique pour l'affichage au
  chargement, calculé sur toutes les images FITS chargées ensemble.
- Changer l'étirement est instantané : rien n'est à recalculer.
- L'**Exposition** est appliquée avant l'étirement.

### 5.7 Annuler, rétablir et projets

**Annuler / rétablir :** **↶ Annuler** et **↷ Rétablir** dans la barre
d'outils, ou **Ctrl+Z** et **Ctrl+Y** (aussi **Ctrl+Maj+Z**). La ligne d'état
indique ce qui a été annulé, p. ex. *« Annulé : Tangage (°) 0 → 10 »*.

- Annulable : chaque curseur, menu et interrupteur des réglages ; l'ajout, le
  retrait et la réorganisation des images ; les résultats d'appariement
  (Apparier, Réapparier, Écarter) ; l'image de référence ; les déplacements
  manuels ; la luminosité ; le recadrage automatique.
- Faire glisser un curseur compte pour **une** étape, quelle que soit la durée.
  Des changements sur deux commandes différentes font toujours deux étapes.
- Les images retirées restent en mémoire pour l'annulation, elles reviennent
  donc aussitôt, avec leurs résultats d'appariement.
- Les 100 dernières étapes sont gardées. Non annulable : ce qui ne touche que
  l'affichage (zoom, vue affichée, superpositions), **RAW via** et **RAW en
  demi-taille**, et l'enregistrement de fichiers.

**Projets :** **⬇ Enregistrer le projet…** (en haut de la colonne de gauche)
écrit un fichier de projet avec l'extension **`.panoproj`**. **⇪ Ouvrir un
projet…** le recharge. Vous pouvez aussi taper le chemin d'un fichier
`.panoproj` dans le champ de chemin et appuyer sur Entrée, ou lancer le
programme avec : `panostitch mon_pano.panoproj`.

Un projet contient :

- les images (chemin complet et chemin relatif au fichier de projet)
- tous les réglages : curseurs, menus, interrupteurs, y compris l'étirement
- les résultats d'appariement, rien n'est donc à apparier à nouveau
- les déplacements manuels, les facteurs de luminosité, l'image de référence
  et le recadrage
- **RAW via** et **RAW en demi-taille**, pour que les images soient chargées
  de la même façon

Il **ne** contient **pas** les images elles-mêmes ; elles sont chargées depuis
leurs fichiers. Si vous déplacez les photos avec le fichier de projet (p. ex.
tout le dossier), elles sont retrouvées par le chemin relatif. Les fichiers
manquants sont signalés et laissés de côté.

Le fichier est du texte lisible (format TOML). Sa taille vient surtout des
points de correspondance enregistrés : environ 50 ko par image.

---

## 6. Se déplacer dans la vue

| Action | Souris | Clavier | Bouton |
|---|---|---|---|
| Déplacer | glisser avec le bouton **gauche** | flèches (vues Image et Paire) | |
| Zoomer | molette vers le haut | `+` | **Zoom +** |
| Dézoomer | molette vers le bas | `-` | **Zoom -** |
| Réinitialiser le zoom | | `F`, `0` ou `Début` | **Ajuster** |
| Sélectionner une image (vue Panorama) | clic dans son contour | | |
| Afficher / masquer contours, numéros, liens | | | interrupteur **superpositions** |
| Déplacer une image (vue Panorama) | **Ctrl + glisser** | flèches (`Maj` = 10 pas) | **← ↑ ↓ →** |
| Annuler | | `Ctrl+Z` | **↶ Annuler** |
| Rétablir | | `Ctrl+Y` ou `Ctrl+Maj+Z` | **↷ Rétablir** |

Les boutons **Image + points clés**, **Correspondances de la paire** et
**Panorama** passent d'une vue à l'autre.

**Autres raccourcis** (survolez un bouton pour voir son raccourci) :

| Touches | Assemblage de panoramas | Empilement d'étoiles |
|---|---|---|
| `Ctrl+Entrée` | **▶ Exécuter toutes les étapes** | **▶ Démarrer** |
| `P` / `Ctrl+P` | `P` : **◐ Aperçu du panorama** | `Ctrl+P` : **◐ Aperçu rapide** |
| `Ctrl+S` | enregistrer le projet (directement dans le fichier ouvert ; la première fois, demande un nom) | idem |
| `Ctrl+Maj+S` | **⬇ Enregistrer le panorama…** | |
| `Ctrl+O` | **⇪ Ouvrir un projet…** | idem |
| `Échap` | | annuler l'empilement en cours |
| `F` | ajuster l'image à la vue | idem |
| `F1` | ce manuel, au chapitre de l'onglet | idem |

Sous macOS, utilisez `Cmd` au lieu de `Ctrl`.

**Fermer la fenêtre** demande d'abord si un empilement est en cours ou si le
travail d'un onglet n'est pas enregistré dans un fichier de projet. Vous
pouvez alors enregistrer le projet, quitter quand même ou annuler.

**Zoom net :** l'aperçu est une image réduite. Quand vous zoomez dans le
panorama et marquez une pause, la zone visible est recalculée à la résolution
de l'écran à partir des images d'origine, pour vérifier les détails fins et
les raccords.

---

## 7. Projections

Un panorama couvre plus qu'une photo plate ne peut montrer, le programme doit
donc *projeter* la vue sur une image plate. Chaque projection déforme quelque
chose :

| Projection | Aspect | Idéal pour | Limites |
|---|---|---|---|
| **rectilinéaire** | une photo ordinaire : les droites restent droites | panoramas étroits (jusqu'à ~100°), architecture, mosaïques au télescope | les bords s'étirent énormément quand la vue s'élargit |
| **cylindrique** | les droites horizontales se courbent, les verticales restent verticales | la plupart des panoramas horizontaux | pas pour les vues hautes (le haut et le bas s'étirent) |
| **sphérique** (équirectangulaire) | comme la cylindrique, avec moins d'étirement vertical | panoramas très larges ou à 360°, visionneuses de panoramas | les droites se courbent |
| **Mercator** | entre cylindrique et sphérique | panoramas larges avec un peu de hauteur | étire près du haut et du bas |
| **stéréographique** | façon fisheye, garde les formes rondes | champs très larges, images créatives | forte courbure |
| **cylindrique transverse** | la cylindrique couchée : les verticales se courbent, les horizontales restent droites | **panoramas verticaux** (de l'horizon au zénith), l'arche de la Voie lactée debout | pas pour les larges balayages horizontaux (ils s'enroulent par le haut et le bas) |
| **Mercator transverse** | comme la cylindrique transverse, moins étirée sur les côtés | panoramas verticaux avec un peu de largeur | comme ci-dessus |
| **fisheye** (équidistant) | image ronde, distance au centre proportionnelle à l'angle | **vues du ciel entier** : inclinez avec le tangage pour mettre le zénith au centre | bords comprimés |
| **fisheye équisolide** | image ronde qui garde les surfaces en proportion | vues du ciel entier, comme rendent la plupart des fisheyes | bords encore plus comprimés |
| **petite planète** | le sol au centre, le ciel autour en anneau | panoramas à 360° avec le sol, créatif | demande un panorama qui descend très bas |
| **tunnel** | le ciel au centre, l'horizon autour | ciels nocturnes à 360°, filés d'étoiles, arbres vus d'en bas | demande un panorama qui monte très haut |
| **orthographique** | un globe vu de l'extérieur | créatif ; montre une moitié de la sphère | bords très comprimés |
| **Hammer** (Hammer–Aitoff) | une carte ovale de toute la sphère aux surfaces relatives justes | panoramas de sphère complète en une image, cartes du ciel entier | formes déformées près des bords |
| **Mollweide** | une carte ovale équivalente, plus plate aux pôles | comme Hammer ; la carte classique du ciel entier en astronomie | formes déformées près des bords |

Si l'horizon fait un sourire ou une moue, ajustez **Tangage (°)**. S'il penche,
ajustez **Roulis (°)**. **Lacet (°)** tourne la vue sur le côté. Pour les vues
fisheye, petite planète et tunnel, le tangage décide de ce qui est au centre.

---

## 8. Fichiers RAW

Pris en charge : **Canon CR2 et CR3, Nikon NEF et NRW, Sony ARW et SRF, DNG,
Fuji RAF, Olympus ORF, Panasonic RW2, Pentax PEF, Samsung SRW**, et d'autres.
La prise en charge exacte des appareils dépend du décodeur ; l'intégré connaît
la plupart des appareils des deux dernières décennies, LibRaw (s'il est
installé) environ 1250.

**RAW via** choisit le décodeur :

| Option | Décodeur | Caractère |
|---|---|---|
| **auto** | le premier qui fonctionne, dans l'ordre ci-dessous ; RawTherapee d'abord si la photo a un fichier `.pp3` | recommandé |
| **built-in** | le décodeur propre au programme | toujours disponible, rapide ; rendu neutre avec les couleurs et la balance des blancs de l'appareil et sans éclaircissement automatique, toutes les images gardent donc la même exposition |
| **libraw** | `dcraw_emu` (LibRaw) | proche de l'intégré ; pour les appareils que l'intégré ne connaît pas |
| **rawtherapee** | `rawtherapee-cli` | **utilise vos retouches RawTherapee** enregistrées dans `photo.NEF.pp3` ; sinon le rendu par défaut de RawTherapee |
| **darktable** | `darktable-cli` | le rendu par défaut de darktable (avec correction d'objectif si disponible) ; le plus lent |
| **dcraw** | `dcraw` | solution de secours ; ne lit pas le CR3 |

Seul l'intégré est toujours disponible ; les autres apparaissent quand le
programme est installé sur votre ordinateur. L'empilement d'étoiles a besoin de
données linéaires, il n'utilise donc que built-in, libraw ou dcraw.

Les décodeurs externes écrivent une image 16 bits dans un dossier temporaire
supprimé ensuite. Rien n'est écrit à côté de vos photos.

**Astuce :** pour des couleurs cohérentes dans tout le panorama, utilisez les
mêmes retouches (le même profil `.pp3`) pour toutes les images, ou utilisez
**built-in** et corrigez les couleurs ensuite.

---

## 9. Fichiers FITS (astronomie)

Les fichiers terminés par **`.fits`, `.fit` ou `.fts`** sont lus directement,
sans logiciel supplémentaire. Pris en charge :

| Contenu | Exemples | Lecture |
|---|---|---|
| **Entiers 8, 16, 32 et 64 bits, flottants 32 et 64 bits** | captures de caméra, résultats d'empilement | mis à l'échelle 0…1 (16 bits non signé : 0…65535) |
| **Couleur (3 plans)** | résultats empilés de Siril, PixInsight, DSS | en RVB |
| **Mono** | caméras monochromes, filtres seuls | en gris |
| **Brut de caméra couleur (Bayer)** | images de ZWO, ToupTek, QHY… avec un en-tête `BAYERPAT` (RGGB, BGGR, GRBG, GBRG) | dématricées ; avec **RAW en demi-taille** un pixel couleur par cellule 2×2 (rapide, propre), sinon interpolées en taille réelle |
| **Ordre des lignes** | `ROWORDER` = `TOP-DOWN` ou `BOTTOM-UP` (le défaut FITS) | l'image est affichée dans le bon sens |

Non pris en charge : **les fichiers compressés par tuiles (`.fz`)**.
Décompressez-les d'abord, p. ex. avec `funpack`. Les parties supplémentaires
d'un fichier (tables, profils ICC) sont ignorées ; la première image est
utilisée.

**Remarques pour les mosaïques de ciel profond :**

- Utilisez le préréglage **mosaïque de ciel profond**. Les tuiles au télescope
  ne font que se décaler et tourner (rotation de champ), ce que le modèle de
  *similitude* décrit exactement.
- **Les tuiles empilées et calibrées** s'assemblent le mieux. Les images
  courtes isolées ont peu d'étoiles visibles et beaucoup de bruit.
- Les images brutes de caméras couleur ne sont pas équilibrées en blanc, elles
  paraissent donc vertes ; l'étirement ne change pas l'équilibre des couleurs.
- Enregistrez en **FITS** pour continuer le traitement (extraction du fond,
  calibration des couleurs, étirement final) dans Siril ou PixInsight. La
  sortie FITS garde les valeurs linéaires.

---

## 10. Enregistrer

Cliquez sur **⬇ Enregistrer le panorama…** et choisissez un nom de fichier. Le
type de fichier suit l'extension :

| Extension | Résultat |
|---|---|
| `.tif` / `.tiff` | 16 bits par canal, le mieux pour la retouche ensuite ; enregistré **tel qu'affiché** (avec exposition et étirement) |
| `.png` | 16 bits par canal ; tel qu'affiché |
| `.jpg` | 8 bits par canal, fichier plus petit ; tel qu'affiché |
| `.fits` / `.fit` / `.fts` | virgule flottante 32 bits, trois plans de couleur ; **linéaire** (exposition et facteurs de luminosité appliqués, mais **sans étirement** ni écrêtage), pour les logiciels d'astronomie |

Quand toutes les images chargées sont des FITS, le nom de fichier proposé se
termine par `.fits`.

**Taille de sortie** (au-dessus du bouton d'enregistrement) fixe la taille du
panorama enregistré, en % de la pleine résolution des photos ; la ligne en
dessous indique la taille en pixels (après un aperçu). Les boutons ¼, ½,
complet et 2× sont des raccourcis. 100 % garde tout le détail ; les RAW chargés
avec **RAW en demi-taille** sont relus en taille réelle à l'enregistrement, ce
qui demande plus de temps et de mémoire. 50 % ou 25 % donnent vite un fichier
plus petit ; au-delà de 100 %, on agrandit (sans nouveau détail).

Sous la taille, une estimation indique la mémoire nécessaire à
l'enregistrement et la taille possible du fichier, à côté de ce qui est libre.
Elle devient orange (⚠) quand cela ne tient pas ; survolez-la pour savoir quoi
faire. L'enregistrement refuse alors avec le même conseil au lieu de remplir la
mémoire, ce qui bloquerait tout l'ordinateur. Le chargement s'arrête tôt, avec
un message, quand la mémoire est presque pleine.

Un grand panorama est calculé par bandes de lignes quand il ne tient pas d'un
seul bloc en mémoire ; la ligne d'état compte alors les bandes. Seul le
panorama fini est gardé entier, dans le format du fichier (3 octets par pixel
en JPEG, 6 en TIFF et PNG 16 bits), et les photos relues en taille réelle sont
gardées en demi-précision. Un TIFF 16 bits de 20 000 × 8000 px demande ainsi
environ 1 Go plus une bande et les photos, là où il en fallait environ 8 Go.
Avec la fusion **intelligent**, chaque bande est calculée avec un peu de
recouvrement, les bandes se raccordent donc sans marche visible.
L'enregistrement n'utilise jamais plus de 85 % de la mémoire.

Les panoramas de plus de 60 000 px de large ou de haut sont refusés. Cela
signifie presque toujours qu'une paire est mal appariée ou que la projection ne
convient pas ; essayez cylindrique ou sphérique.

Pendant que l'assemblage travaille, la ligne d'état affiche le temps écoulé.
Après l'enregistrement, un petit **rapport** sous le bouton d'enregistrement
résume : combien de photos sont dans le panorama (et combien n'ont pas été
appariées), le temps d'assemblage (chargement, appariement et aperçus depuis
l'ajout de la première photo), le temps d'enregistrement, la taille en pixels
et le fichier avec sa taille, ou l'erreur en cas d'échec. × le ferme.

---

## 11. Dépannage

| Problème | Que faire |
|---|---|
| **Aucune fenêtre n'apparaît** | Sous Windows, un message en donne la raison ; sous Linux et macOS, lancez le programme depuis un terminal et lisez l'erreur. La fenêtre demande OpenGL 2.0 ou plus récent : mettez à jour le pilote graphique (sur les vieux PC, celui du fabricant de la puce graphique). |
| **Le programme s'est fermé de façon inattendue** | Au démarrage suivant, la ligne d'état le signale. Les détails sont dans `crash.log` dans le dossier de réglages (Linux `~/.config/panostitch`, Windows `%APPDATA%\panostitch`, macOS `~/Library/Application Support/panostitch`) ; joignez-le quand vous signalez le problème. |
| **Commandes coupées sur le côté** | Élargissez en faisant glisser le bord entre un panneau latéral et l'image. |
| **« Occupé – attendez… »** | Une étape est encore en cours ; attendez que la ligne d'état change. |
| **Une image est ignorée au chargement** | La ligne d'état indique le fichier et la raison. Pour un RAW, essayez un autre convertisseur dans **RAW via**. Pour un FITS `.fz`, décompressez d'abord le fichier. |
| **Le panorama est presque noir** | Photos : cliquez sur **Exposition automatique** ou montez **Exposition (EV)** (les photos de nuit demandent souvent +1,5 à +4), ou retouchez les photos dans RawTherapee (leurs fichiers `.pp3` sont alors utilisés). FITS et autres données linéaires : cliquez sur **Étirement automatique**. |
| **Les images FITS paraissent noires ou blanches au chargement** | L'étirement automatique au chargement se base sur toutes les images FITS ; une image très sur- ou sous-exposée peut le fausser. Retirez ces images, puis cliquez sur **Étirement automatique** (ou **Réinitialiser l'étirement** et réglez à la main). |
| **Les images FITS paraissent vertes** | Les images brutes de caméras couleur ne sont pas équilibrées en blanc. Empilez-les et calibrez les couleurs d'abord, ou corrigez la couleur du résultat dans un logiciel d'astronomie. |
| **Peu de points clés jaunes** | Baissez le **Seuil des coins** ; montez **Points clés max.** ; montez **Taille de détection (px)**. |
| **Points clés surtout sur l'herbe, le ciel ou le bruit** | Montez **Débruitage (σ px)** à 2–3. |
| **Images marquées UNLINKED** | Regardez-les : chevauchent-elles vraiment une autre image ? Montez **Test du ratio** (0,85–0,9) et **Distance max.** (0,4), baissez **Inliers min.**, montez **Débruitage (σ px)**, puis **Apparier** à nouveau. Si cela échoue encore, activez **montrer les images non reliées** et placez-les avec **Ctrl+glisser**. |
| **Un trou au zénith, ou des images de ciel seul non reliées (UNLINKED)** | Utilisez les fichiers RAW (leur focale et leur heure de prise sont nécessaires), gardez **toutes les paires** et cliquez de nouveau sur **Apparier** : les images de ciel seul sont alors appariées par leurs étoiles. Chaque image de ciel a besoin de quelques dizaines d'étoiles et d'un bon recouvrement avec ses voisines. |
| **Rangées vers le bas (ou le nadir) non reliées** | Le sol photographié de près se décale au moindre mouvement de l'appareil (parallaxe), et l'herbe de nuit n'est presque que du bruit. Prenez les rangées du sol avec une tête panoramique qui tourne autour de l'objectif, ou masquez le nadir après coup. |
| **Nuages, eau ou autres choses qui bougent** | Ils changent d'une prise à l'autre et ne peuvent pas être appariés de façon fiable (les images ne montrant que des nuages restent en général non reliées). Photographiez ces parties vite, l'une après l'autre, ou placez-les à la main. |
| **Une mauvaise paire est utilisée** (lignes dans tous les sens, images au mauvais endroit) | Sélectionnez-la dans le menu **Paire** et cliquez sur **Écarter la paire**. Avec **affiner** activé, la plupart de ces paires sont rejetées automatiquement. |
| **Fantômes (objets dédoublés)** | Gardez **affiner** activé et utilisez **Fusion → intelligent** (par défaut). Les problèmes restants viennent de choses qui ont changé entre les prises (p. ex. une lampe allumée sur une seule photo), ou de la **parallaxe** : les objets proches se décalent par rapport au fond quand l'appareil ne tourne pas autour de son objectif (le « point nodal »). |
| **Marches de luminosité visibles** | **Égaliser**, puis affinez image par image avec **Luminosité (sél.)**. |
| **Horizon courbé** | Ajustez **Tangage (°)** (et **Roulis (°)** s'il penche). |
| **Haut ou bas très étiré** | Utilisez la projection **sphérique** ; la ligne d'état la propose quand le panorama monte ou descend très loin. |
| **Panorama comprimé ou étiré** | Ajustez **Focale (× auto)**. |
| **Panorama énorme et déformé** | Une mauvaise paire (vérifiez les paires), ou utilisez **sphérique** / **cylindrique** au lieu de rectilinéaire. |
| **Lent ou mémoire insuffisante** | Activez **RAW en demi-taille** avant le chargement ; baissez **Taille de l'aperçu (px)**. Les images en pleine résolution demandent environ 12 octets par pixel chacune, environ 300 Mo pour une image de 24 Mpx. Avec beaucoup d'images, l'appariement *toutes les paires* prend plus de temps ; *grille* est plus rapide si vous connaissez la disposition. |
| **Le recadrage automatique ne garde qu'une fine bande** | Le panorama est courbé. Redressez-le d'abord avec **Tangage (°)** et **Roulis (°)** (et choisissez une projection adaptée), puis **Recadrage automatique** à nouveau. |
| **Un projet s'ouvre sans certaines images** | La ligne d'état liste les fichiers manquants. Ils ont été déplacés ou renommés, et pas avec le fichier de projet. Remettez-les en place, ou ajoutez-les à nouveau et appariez. |
| **Annuler ne fait rien** | L'annulation porte sur des changements stabilisés ; attendez un instant après le dernier changement. Pendant une étape (*Occupé*), l'annulation attend. |
| **Tout est lent** | Activez **RAW en demi-taille** avant le chargement (RAW et FITS de caméras couleur) ; baissez **Taille de l'aperçu (px)**. Dans l'onglet d'empilement, **Demi-taille** sous Conversion RAW. |

---

## 12. Conseils pour de bons panoramas

- **Faites chevaucher** les prises voisines de 30–50 %, aussi entre les
  rangées. Pour les mosaïques au télescope, 15–25 % suffit en général.
- **Tournez l'appareil** autour de l'objectif, pas autour de vous. Utilisez un
  trépied si possible, idéalement avec une tête panoramique.
- **Verrouillez l'exposition, la balance des blancs et la mise au point** (mode
  manuel), pour que toutes les images concordent.
- **Scotchez la bague de zoom** pour les panoramas de nuit : tourné vers le
  haut, un zoom peut glisser. PanoStitch s'en accommode, mais une focale fixe
  s'apparie le mieux.
- Évitez les objets en mouvement dans les zones de recouvrement, ou prenez-les
  sur une seule image.
- **Gardez l'appareil de niveau.** Un appareil penché courbe l'horizon (se
  corrige avec le tangage et le roulis).
- Pour les longues séries, vérifiez d'abord le résultat avec **RAW en
  demi-taille** activé.
- N'enregistrez pas le résultat dans le dossier des images sources si vous
  chargez les images par dossier. Les fichiers nommés `panorama…` sont ignorés,
  les autres noms non.

---

## 13. Empilement d'étoiles

Le deuxième onglet, **Empilement d'étoiles (paysage / filés)**, combine de
nombreuses poses de la même scène en une seule image : un ciel peu bruité aux
étoiles nettes, ou des filés d'étoiles. Il est fait pour l'astrophotographie de
paysage : l'appareil est sur un trépied, le ciel bouge d'une image à l'autre,
le premier plan non.

Il fonctionne comme Sequator, avec ce qui manque à Sequator : offsets et darks
de flat, sortie linéaire 32 bits, une image de premier plan séparée, des filés
d'étoiles sans trous, la détection automatique du ciel et pas de boîte noire :
chaque image est mesurée et notée, un graphique montre les mesures et un
journal liste chaque étape avec ce qu'elle a fait et sa durée.

```
┌──────────────────────┬──────────────────────────────────────┬─────────────────────┐
│ input list           │ [Base image] [Result] [Frame report] │ Post-processing     │
│  Star images, Base,  │ [Fit] [Open project…] [Save project…]│  Background extr.   │
│  calibration, Fore-  │                                      │  Color calibration  │
│  ground, Output      │                                      │                     │
│ Add Remove Clear     │   base image (draw the sky here),    │  Colour noise       │
│ Use as base          │   result, or frame report            │  Light pollution    │
│ settings list        │                                      │  Brightness, Tone   │
│ options of the       │                                      │  Color space        │
│ highlighted entry    │                                      │  [Save result…]     │
│                      │                                      │  [Use in Stitcher]  │
│ [Quick prev.] [Start]│  status line                         │                     │
└──────────────────────┴──────────────────────────────────────┴─────────────────────┘
```

**Cliquez sur une entrée** de la liste des entrées ou de la liste des
réglages : ses options et une explication apparaissent sous les listes.

**Les préréglages** (au-dessus de la liste des réglages) règlent tout pour un
sujet typique en un clic : **Paysage avec Voie lactée** (figer le sol, ciel
détecté automatiquement, correction de distorsion, pollution lumineuse),
**Filés d'étoiles** (trous comblés) et **Ciel profond (télescope)** (tout le
ciel, extraction du fond, calibration des couleurs, étirement automatique).
Chaque réglage peut encore être changé. Pendant un passage, une barre de
progression sous la vue montre l'étape, le temps écoulé et le temps restant
estimé. À la fin, un petit **rapport** apparaît en haut du panneau de droite :
combien d'images ont été empilées sur combien (écartées et exclues comptées),
les images de calibration, la durée, la taille du résultat et le fichier
enregistré avec sa taille, ou ce qui n'a pas marché. × le ferme.

### 13.1 Démarrage rapide

1. Sélectionnez **Brutes**, cliquez sur **Ajouter…** et choisissez les photos
   d'étoiles (ou tapez le chemin d'un fichier ou d'un dossier dans le champ de
   chemin et appuyez sur Entrée).
2. Sélectionnez **Zone du ciel**, choisissez **Ligne de séparation** et tracez
   une ligne le long de l'horizon dans la vue de l'image de référence, ou
   cliquez sur **◎ Détecter le ciel** et corrigez le masque au pinceau. Les
   étoiles ne sont cherchées que dans le ciel.
3. Cliquez sur **◐ Aperçu rapide** : un petit résultat rapide à partir de 10
   images au plus, pour vérifier les réglages. Rien n'est enregistré.
4. Vérifiez la ligne **Enregistrer sous** sous la liste des entrées : tapez un
   nom de fichier ou choisissez l'un des noms proposés dans la liste à côté,
   choisissez le type de fichier dans la deuxième liste, et le dossier avec
   **Dossier…** (par défaut : `stacked.tif` à côté de la première image
   d'étoiles). Si le fichier existe déjà, la ligne le signale ; **nom libre
   suivant** ajoute un numéro. En dessous, une estimation indique l'espace
   dont les images ont besoin pendant l'empilement (selon leur nombre et leur
   taille) et si elles restent en mémoire ou sur disque ; elle devient orange
   (⚠) s'il n'y en a pas assez. Cliquez ensuite sur **▶ Démarrer**.
5. Réglez le post-traitement à droite ; le résultat se met à jour aussitôt.
   Cliquez sur **⬇ Enregistrer le résultat…** pour l'enregistrer à nouveau
   avec les nouveaux réglages.

### 13.2 Fichiers d'entrée

| Entrée | Quoi photographier |
|---|---|
| **Brutes** (*Images d'étoiles* en mode simple) | Les prises du ciel : même focale, même ouverture, même exposition. Le RAW est préférable ; TIFF, FITS ou XISF seulement si les données sont encore linéaires (non retouchées). **Unifier l'exposition** égalise des images prises dans d'autres conditions. |
| **Image de référence** | L'image sur laquelle les autres sont alignées. Vide = la brute du milieu. Choisissez-en une autre dans la liste à côté de **Image de référence**, ou choisissez une brute dans sa liste et cliquez sur **Utiliser comme référence**. |
| **Darks** (*Images de bruit* en mode simple) | Images noires : bouchon sur l'objectif, même ISO et même temps de pose, prises juste avant ou après les images d'étoiles. Enlèvent le bruit thermique et les pixels chauds. |
| **Offsets** (*Images d'offset* en mode simple) | Pose la plus courte, bouchon sur l'objectif, même ISO. |
| **Flats** (*Images de vignetage* en mode simple) | Images de PLU : une surface blanche uniformément éclairée, même focale et même ouverture. Corrigent les coins plus sombres et les ombres de poussière. |
| **Darks de flat** (*Images dark de flat* en mode simple) | Darks pour les flats : même ISO et même temps de pose que les flats. |
| **Image de premier plan** | Facultatif : une prise du sol depuis la même position du trépied – à l'heure bleue, en pose longue ou à ISO bas. Elle remplace le premier plan empilé (bruité, sombre) sous la zone du ciel. **Accorder la luminosité au sol empilé** l'ajuste au sol empilé ; **Exposition (EV)** l'éclaircit ou l'assombrit à partir de là. Nécessite une zone du ciel. |
| **Sortie** | Se règle sur la ligne **Enregistrer sous** : nom, type et dossier. `.tif` / `.png` (16 bits), `.jpg` (8 bits), ou `.fits` / `.xisf` (virgule flottante 32 bits, linéaire, pour la suite du traitement dans Siril, PixInsight, etc.). |

Toutes les images de calibration sont facultatives. Même quelques darks
aident beaucoup à ISO élevé.

**Les listes de fichiers :** chaque groupe montre ses fichiers dans une liste
à côté de son nom, des centaines de brutes ne demandent donc aucun défilement.
Choisissez-y un fichier pour le voir dans l'aperçu ; **Retirer** et **Utiliser
comme référence** agissent sur ce fichier, **Vider** vide tout le groupe.

**Les flats bruités** (trop peu nombreux, ou trop sombres dans une couleur,
p. ex. avec une source lumineuse bleue) sont lissés automatiquement ; le
journal le signale, et à quel point ils étaient bruités. Sinon, leur bruit
serait imprimé sur chaque image et deviendrait de fines stries en suivant la
dérive des étoiles.

**Fichiers temporaires :** aucun n'est écrit pour les images intermédiaires.
Les images sont gardées en mémoire ou, si elles prennent plus de la moitié de
la mémoire libre, dans un fichier temporaire dans le dossier de sortie,
supprimé dès la fin du passage (il n'est même pas visible pendant). Le journal
dit lequel. S'il n'y a pas assez de place, le passage s'arrête avec un message
avant de commencer : activez **Demi-taille** (un quart de l'espace) ou
enregistrez la sortie sur un disque plus libre. Les maîtres (dark, offset,
flat) ne sont pas enregistrés non plus.

### 13.3 Réglages
| Réglage | Effet |
|---|---|
| **Composition** | **Aligner les étoiles** : les images sont alignées sur les étoiles et combinées. **Accumulation** en fait la moyenne ; **Meilleurs pixels** (par défaut) écarte les valeurs aberrantes (avions, satellites) ; **Figer le sol** n'aligne que le ciel, ciel et premier plan restent donc nets (nécessite une zone du ciel) ; **Aligner seulement** écrit juste les images alignées. **Filés d'étoiles** : pas d'alignement, le ciel garde la valeur la plus brillante de chaque pixel. **Fondu des filés** : **Apparition** (les filés commencent faibles, l'effet de mouvement de Sequator), **Disparition** ou **Les deux bouts** (effilé comme une comète). **Combler les trous entre images** : les courtes pauses de l'intervalle de l'appareil sont comblées le long du mouvement mesuré des étoiles, les filés sont donc des lignes continues au lieu de tirets. |
| **Zone du ciel** | Où se trouve le ciel. **Tout le ciel** : toute l'image est du ciel (pas de premier plan, p. ex. prises au télescope ou de ciel profond). **Ligne de séparation** : tracez une ligne le long de l'horizon. **Dégradé** : la ligne plus une transition douce. **Masque irrégulier** : peignez le ciel avec le bouton gauche, effacez avec le droit ; **Taille du pinceau** (curseur ou molette). **◎ Détecter le ciel** peint un premier masque d'après la luminosité et les étoiles – approximatif, vérifiez-le donc et corrigez-le au pinceau. **Adoucir le masque** adoucit le bord du masque, seulement à l'intérieur du ciel, pour que le premier plan ne reçoive rien du ciel (pas de halo le long de l'horizon). **Montrer les étoiles détectées** entoure les étoiles trouvées par le dernier passage dans l'image affichée. |
| **Sigma clipping** | La sévérité du retrait des valeurs aberrantes (bas / haut × σ). 2–3 est habituel ; plus bas en retire davantage mais laisse plus de bruit. Demande au moins 4 images, idéalement 10+. |
| **Rejet des images** | Écarte les images nettement moins bonnes que la typique : nuages ou rosée (étoiles plus faibles), peu d'étoiles, étoiles floues ou allongées (mise au point, vent, trépied heurté). |
| **Conversion RAW** | Balance des blancs (appareil, moyenne de l'image, lumière du jour), dématriçage (AHD par défaut ; DHT ou AAHD donnent souvent moins de bruit chromatique à ISO élevé) et **Demi-taille** (4× moins de pixels, bien plus rapide). |
| **Réduire le bruit dynamique** | Trouve et enlève les pixels chauds sans darks. |
| **Réduire les effets de distorsion** | Corrige la distorsion de l'objectif qui laisse de courts filés dans les coins des longues séquences : **Objectif**, **Complexe** (ultra grands-angles, réfraction près de l'horizon) ou **Auto**. |
| **Fusionner 4 pixels** | Additionne les pixels par 2×2 : plus lumineux et moins bruité, moitié moins large et haut. |
| **Timelapse** | Empile chaque groupe de N images consécutives en une, ce qui donne une séquence numérotée d'images peu bruitées dans un dossier à côté de la sortie. |

### 13.4 Les vues

- **Image de référence** : l'image sur laquelle les autres sont alignées ; la
  zone du ciel se dessine ici. Choisissez un fichier dans la liste d'un groupe
  pour le voir.
- **Résultat** : l'empilement après le post-traitement. Faites glisser pour
  déplacer, molette pour zoomer, **Ajuster** pour tout voir.
- **Rapport des images** : un graphique de chaque mesure par image, chacune
  divisée par sa médiane, pour qu'une mauvaise image ressorte (rouge =
  écartée, bleu = l'image de référence) ; en dessous, une ligne par image
  d'étoiles : étoiles trouvées, taille des étoiles (FWHM), élongation, niveau
  du ciel, lumière (luminosité des étoiles par rapport à l'image de
  référence), erreur d'alignement, si l'image a été utilisée et sinon
  pourquoi. Sous le tableau, le **journal du traitement** : chaque étape du
  passage, ce qu'elle a fait (calibration, zone du ciel, images utilisées,
  distorsion, comblement des trous, premier plan, post-traitement) et sa durée.
  **Cliquez sur une image** dans le graphique ou le tableau pour l'exclure (ou
  la réinclure), puis à nouveau sur **▶ Démarrer**.

### 13.5 Post-traitement

Appliqué aussitôt au résultat empilé ; **▶ Démarrer** enregistre avec ces
réglages, **⬇ Enregistrer le résultat…** enregistre à nouveau.

| Réglage | Effet |
|---|---|
| **Extraction du fond** | Soustrait un modèle lisse du fond du ciel (dégradés de pollution lumineuse ou de clair de lune). |
| **Calibration des couleurs** | Rend le fond du ciel gris neutre et l'étoile moyenne blanche. |
| **Réduire le bruit chromatique** | Lisse seulement les couleurs, en enlevant les taches colorées des images à ISO élevé. |
| **Pollution lumineuse** | **Ciel profond** : enlève une lueur lisse. **Irrégulière** : suit la lueur tuile par tuile (dômes de lumière des villes en grand-angle). **Intensité** règle la quantité retirée. Corrigez d'abord le vignetage (flats), sinon c'est pire. |
| **Renforcer la lumière des étoiles** | Fait ressortir les étoiles du fond du ciel sans en amplifier le bruit. |
| **Luminosité automatique** | Place le ciel à un niveau agréable sans écrêter les étoiles les plus brillantes. |
| **Grande plage dynamique** | Comprime les étoiles brillantes et les lumières au lieu de les écrêter. |
| **Tonalité** | **courbe sRGB** (comme un appareil photo), **Étirement** (point noir, tons moyens, point blanc ; automatique ou à la main) ou **Linéaire** (sans courbe). |
| **Espace colorimétrique** | sRGB ou Adobe RGB pour la sortie TIFF et JPEG. |
| **Linéaire (pour traitement)** | Désactive tout et enregistre un FITS linéaire 32 bits : ce qu'attendent Siril ou PixInsight. |
| **⇄ Utiliser dans l'Assemblage (panneau de panorama)** | Ajoute le résultat enregistré à l'onglet Assemblage de panoramas, comme un panneau d'un panorama (voir 13.7). |

### 13.6 Projets

**⬇ Enregistrer le projet…** écrit un fichier `.stackproj` avec les listes de
fichiers, la zone du ciel et chaque réglage. **⇪ Ouvrir un projet…** (ou
`panostitch nuit.stackproj`) le restaure. Les fichiers sont retrouvés par leur
chemin complet, ou relativement au fichier de projet si le dossier a été
déplacé.

### 13.7 Panoramas de panneaux empilés

Un panorama de la Voie lactée peu bruité : prenez plusieurs images à chaque
position du panorama (avec un recouvrement entre positions), puis :

1. Empilez les images de la première position (Démarrer), puis cliquez sur
   **⇄ Utiliser dans l'Assemblage (panneau de panorama)**.
2. Remplacez les images d'étoiles par celles de la position suivante, donnez
   un nouveau nom à la sortie, Démarrer, à nouveau **⇄ Utiliser dans
   l'Assemblage (panneau de panorama)** ; et ainsi de suite.
3. Dans l'onglet Assemblage de panoramas, **▶ Exécuter toutes les étapes**
   assemble les panneaux empilés.

Enregistrez chaque panneau en `.tif` ; le même post-traitement pour tous les
panneaux garde leur luminosité et leurs couleurs identiques.

### 13.8 Conseils

- Prenez au moins 10–20 images d'étoiles avec les mêmes réglages, plus 10–20
  darks juste après, sans bouger l'appareil.
- Réglez la zone du ciel avec soin : les arbres et bâtiments comptés comme
  ciel sont flous à cause de l'alignement. **◎ Détecter le ciel** est un bon
  départ, pas le dernier mot.
- Regardez le Rapport des images après un aperçu rapide : le graphique montre
  tout de suite quelles images sont moins bonnes (nuages, vent, rosée).
- Utilisez **Figer le sol** quand le premier plan compte, **Meilleurs pixels**
  sinon.
- Pour traiter dans d'autres programmes, enregistrez en `.fits` et activez
  **Linéaire (pour traitement)**.
- Depuis un terminal, `panostack` empile sans la fenêtre ; `panostack --help`
  liste ses options (voir [Utilisation sans la fenêtre](#14-utilisation-sans-la-fenêtre)).

---

## 14. Utilisation sans la fenêtre

**Empilement d'étoiles depuis un terminal :** `panostack` fait le même
empilement que l'onglet Empilement d'étoiles. `panostack --help` liste toutes
les options. Exemples :

```
panostack --mode freeze --horizon 0.62 --darks darks/ -o nuit.tif lights/
panostack --mode trails --fade in --fill-gaps --auto-sky -o files.jpg lights/
panostack --preview lights/            # vérification rapide, rien n'est enregistré
```

Le rapport des images et le journal du traitement sont affichés à la fin.

**Assembler depuis votre propre programme Rust :** ajoutez PanoStitch comme
dépendance (`panostitch = { path = "…/PanoStitch" }` dans votre `Cargo.toml`),
puis :

```rust
use panostitch::io;
use panostitch::stitcher::{self, Projection, StitchParams};

fn main() -> Result<(), String> {
    let paths = vec!["a.NEF".to_string(), "b.NEF".into(), "c.NEF".into()];
    let p = StitchParams { projection: Projection::Spherical, ..Default::default() };
    let pano = stitcher::stitch_files(&paths, &p, true, &mut |m| println!("{m}"))?;
    io::save_panorama("pano.tif", &pano)
}
```

`examples/scripted.rs` est un exemple complet
(`cargo run --release --example scripted -- a.NEF b.NEF c.NEF`).
`stitch_files(paths, params, raw_half, log)` charge et assemble ;
`stitch(images, params, log)` assemble des images déjà en mémoire.
Les champs de `StitchParams`, avec leurs valeurs par défaut :

| Champ | Par défaut | Signification |
|---|---|---|
| `layout` | Layout::All | quelles paires apparier : `All`, `Sequential`, `Grid` |
| `cols`, `snake` | 0 (toutes les images), `false` | disposition en grille |
| `refine` | `true` | affinage global |
| `feature_scale` | `0.5` | échelle de détection |
| `nkeypoints` | `5000` |  |
| `denoise` | `1.5` |  |
| `ratio` | `0.8` |  |
| `max_distance` | `0.35` |  |
| `model` | `Model::Homography` | `Homography`, `Affine`, `Similarity`, `Translation` |
| `ransac_thresh` | `5.0` |  |
| `min_inliers` | `20` |  |
| `projection` | `Projection::Cylindrical` | n'importe laquelle des [Projections](#7-projections), p. ex. `Fisheye`, `LittlePlanet`, `Mollweide` |
| `blend` | `Blend::Smart` | `Smart`, `Feather`, `Seam`, `Average` |
| `sharpness` | `3.0` | mode fondu : plus haut = transitions plus étroites |

**Mosaïque FITS avec un TIFF étiré et un résultat FITS linéaire :**

```rust
use panostitch::stitcher::{Model, Projection, StitchParams};

let p = StitchParams { model: Model::Similarity, refine: false,
                       projection: Projection::Rectilinear, denoise: 0.75, ..Default::default() };
let mosaic = stitcher::stitch_files(&tiles, &p, false, &mut |_| {})?;
io::save_panorama("mosaic.fits", &mosaic)?;                          // linéaire
let (black, mid, white) = stitcher::auto_stretch(&mosaic, 0.25, 2.8);
io::save_panorama("mosaic.tif", &stitcher::hist_stretch(&mosaic, black, mid, white))?;
```

Empilement d'étoiles depuis le code : remplissez un `stacker::StackConfig` (à
partir de `StackConfig::default()`) et appelez
`stacker::stack(&config, false, &progress, None)`.

---

## 15. Fichiers

| Fichier | Rôle |
|---|---|
| `panostitch` (`panostitch.exe`) | Le programme (la fenêtre). |
| `panostack` (`panostack.exe`) | Empilement d'étoiles en ligne de commande (`panostack --help`). |
| `*.panoproj`, `*.stackproj`, `*.tlproj` | Vos fichiers de projet (où que vous les enregistriez). |
| `ui.toml` dans le dossier de réglages | Les interrupteurs d'apparence et la langue (voir [Lancer le programme](#2-lancer-le-programme)). |
| `remembered/`, `recent_projects.txt`, `last_folder.txt` dans le dossier de réglages | Les réglages gardés pour la session suivante, les projets récents, le dossier où s'ouvrent les boîtes de dialogue. Supprimez-les pour repartir de zéro. |
| `docs/manual_fr.md` | Ce manuel (français). |
| `docs/manual_en.md`, `docs/manual_cs.md`, `docs/manual_de.md`, `docs/manual_es.md`, `docs/manual_pl.md` | Le manuel en anglais, tchèque, allemand, espagnol et polonais. |

Code source (pour compiler le programme vous-même) :

| Chemin | Contenu |
|---|---|
| `Cargo.toml` | Le projet et les bibliothèques qu'il utilise. |
| `src/image.rs`, `src/io/` | Images ; chargement et enregistrement (JPEG, PNG, TIFF, BMP, RAW, FITS, XISF). |
| `src/stitcher/` | Assemblage : points clés, appariement, placement, affinage, projections, fusion. |
| `src/stacker/` | Empilement d'étoiles. |
| `src/finish.rs` | Post-traitement : géométrie, étirements (MTF, arcsinh, GHS), balance des blancs, lumière, courbes, couleur, détail. |
| `src/gui/` | La fenêtre : les quatre onglets, les boîtes de dialogue, le manuel intégré et les textes de la fenêtre en six langues (`i18n_*.rs`). |
| `src/bin/` | Les deux programmes, `panostitch` et `panostack`. |
| `tests/` | Tests : empilement sur des scènes nocturnes synthétiques, assemblage, post-traitement, formats de fichier et les onglets de la fenêtre. Lancez-les avec `cargo test --release`. |
| `examples/scripted.rs` | Utiliser l'assemblage depuis votre propre code Rust. |

---

## 16. Post-traitement

*Nouveau en 0.4 (en développement).* L'onglet **Post-traitement** fait les
dernières étapes sur une image empilée ou assemblée, pour que vous n'ayez pas
besoin d'un autre programme : rotation et recadrage, étirement des données
linéaires, balance des blancs, lumière, courbes, couleur, bruit, étoiles,
netteté et enregistrement à la taille voulue.
**Charger une image :** **Ouvrir…** (Ctrl+O) ou tapez un chemin et appuyez
sur Entrée ; **✎ Utiliser dans Post-traitement** dans l'onglet Empilement
d'étoiles ; ou **✎ Post-traiter** dans l'assemblage après l'enregistrement
d'un panorama. Les fichiers FITS et XISF sont traités comme des données
linéaires et commencent avec l'étirement automatique ; JPEG, TIFF et PNG sont
traités comme des images finies (sans étirement). **Données linéaires (à
étirer)** change cela à la main.

L'aperçu travaille sur une copie de 2000 px au plus, il suit donc les curseurs
presque aussitôt. **⬇ Enregistrer** traite l'image entière en arrière-plan
avec exactement les mêmes réglages ; les valeurs que le programme mesure
(étirement automatique, fond neutre) sont prises sur l'aperçu, le fichier
enregistré ressemble donc à l'aperçu.

Les étapes s'exécutent toujours dans cet ordre, de haut en bas à gauche :

| Section | Effet |
|---|---|
| **Rotation et recadrage** | ⟲ ⟳ quarts de tour, **Miroir**, **Rotation (°)** de n'importe quel angle (fin près de 0°, comme lacet/tangage/roulis dans l'assemblage), **Supprimer les coins vides** après la rotation, recadrage de chaque côté ou recadrage à une forme (3:2, 4:3, 16:9, 4:5, 1:1, 2:1). |
| **Étirement** (données linéaires) | D'abord **Extraction du fond** et **Calibration des couleurs** (toutes deux facultatives, sur les données linéaires), puis la méthode d'étirement (ci-dessous). |
| **Balance des blancs** | Sur la lumière linéaire, avant l'étirement. **◎ Mesurer le gris**, puis tracez un rectangle sur quelque chose qui doit être neutre (fond du ciel, un nuage) : les multiplicateurs de couleur en sont déduits. **Fond neutre** rend gris la moitié la plus sombre. **Température** et **Teinte** affinent. Les images finies sont sorties de leur courbe sRGB pour cela puis y sont remises, la balance est donc juste à toutes les luminosités. |
| **Lumière** | **Exposition (EV)**, **Contraste**, **Hautes lumières** (vers 75 %), **Tons moyens** (50 %), **Ombres** (25 %), **Blancs** et **Noirs** (les extrémités), comme dans les éditeurs photo. |
| **Courbes** | Une courbe pour toutes les couleurs (**RVB**) et une pour chacune : **Rouge**, **Vert**, **Bleu**. Cliquez pour ajouter un point, faites-le glisser pour le déplacer, clic droit pour le retirer. L'histogramme derrière la courbe est celui de ce qui y entre. Un • marque les courbes modifiées. |
| **Couleur** | **Saturation** (0 = noir et blanc) et **Vibrance** (les couleurs ternes plus que les vives). |
| **Détail** | **Réduction du bruit** (lisse les zones unies, garde contours et étoiles), **Bruit chromatique**, **Réduction des étoiles** (étoiles plus petites), **Netteté** et son rayon. Les rayons sont en pixels de l'image entière, l'aperçu correspond donc au résultat enregistré. |
| **Ciel et premier plan** | Des rendus propres au ciel et au premier plan, en plus de tout ce qui précède, à travers un masque (voir plus bas). |

**Méthodes d'étirement :**

| Méthode | Usage |
|---|---|
| **Désactivé** | L'image est déjà finie. |
| **courbe sRGB** | Comme le ferait un appareil photo : pour des données linéaires qui n'ont besoin que du gamma normal. |
| **MTF, automatique** | Point noir juste sous le fond du ciel, tons moyens pour que le fond arrive à la luminosité **Fond** (comme l'AutoStretch de Siril / le STF de PixInsight). **Chaque couleur à part** (par défaut) mesure rouge, vert et bleu séparément, pour que la dominante d'un empilement d'appareil photo ne cache pas l'image ; les données elles-mêmes ne changent pas. **Auto → manuel** continue à la main à partir des valeurs mesurées. |
| **MTF, manuel** | Point noir, tons moyens et point blanc (fonction de transfert des tons moyens). |
| **Arcsinh** | Sinus hyperbolique inverse : étirement fort qui garde la couleur des étoiles. **Facteur d'étirement** 1–10000 ; les empilements profonds au fond faible en demandent plusieurs milliers. |
| **Hyperbolique généralisé (GHS)** | **Étirement ln(D+1)** (force), **Intensité locale b** (à quel point l'étirement se concentre autour du point de symétrie), **Point de symétrie** (là où l'étirement est le plus fort : placez-le sur le fond du ciel), **Protéger les ombres** et **Protéger les hautes lumières** (droite en dessous / au-dessus). |

Pour Arcsinh et GHS : **Inverse** utilise la courbe inverse (comprime au lieu
d'étirer, p. ex. pour défaire un étirement trop fort) ; **Garder la couleur des
étoiles** étire la luminosité et garde la couleur de chaque pixel (désactivé :
chaque couleur à part, ciel plus saturé, étoiles plus pâles) ; **Points du
fond** place le point noir juste sous le fond de la couleur la plus sombre et,
pour GHS, le point de symétrie sur le fond.

**Affichage** (au-dessus de l'aperçu) ne change que la façon dont l'aperçu est
montré, jamais l'image enregistrée, comme les modes d'affichage de Siril :
**Linéaire (tel qu'enregistré)**, **Logarithme**, **Racine carrée**, **Au carré**,
**Asinh**, **AutoStretch**, **Égalisation d'histogramme**. Utile pour regarder
dans les ombres ou vérifier le fond sans rien changer. « affichage seul » est
indiqué tant que ce n'est pas Linéaire.

**◐ Comparer** (maintenez-le, ou maintenez B) montre le point de départ :
tourné, recadré et étiré, rien de plus.

**Enregistrer** (à droite) : **Taille** (réelle, ou un plus grand côté comme
2048 px pour les forums), nom et type de fichier, **Qualité JPEG**. La ligne
en dessous indique la mémoire nécessaire à l'enregistrement. FITS / XISF
reçoivent l'image finie (étirée) en virgule flottante 32 bits. **Garder les
réglages pour l'image suivante** (à gauche) donne le même rendu à plusieurs
images, p. ex. les panneaux d'un panorama.

| Touche | Action |
|---|---|
| Ctrl+O | Ouvrir une image |
| Ctrl+S | Enregistrer (sous le nom de fichier affiché) |
| Ctrl+Maj+S | Enregistrer sous… |
| F | Ajuster l'aperçu |
| B (maintenu) | Comparer avec le point de départ |
| Ctrl+Z, Ctrl+Y | Annuler, rétablir |

**Ciel et premier plan** (fermé au départ) : un paysage sous les étoiles
demande souvent de traiter les deux différemment, p. ex. moins de bruit et des
étoiles plus petites dans le ciel, un premier plan plus clair et plus chaud.
Peignez le masque avec **✎ Peindre** (bouton gauche = ciel, bouton droit =
premier plan, molette = taille du pinceau) ou laissez **◎ Détecter le ciel** le
trouver (ciel en haut ; corrigez-le au pinceau ; pour un fisheye circulaire,
utilisez **◯ Cercle fisheye**, chapitre 17), avec **Tout le ciel**,
**Inverser** et **Vider** pour recommencer. Choisissez ensuite **Ciel** ou
**Premier plan** et réglez sa lumière, sa température, sa couleur et son
détail. **Bord doux** fond les deux rendus à la frontière ; **Montrer le
masque** teinte le ciel en bleu et le premier plan en orange (toujours affiché
en peignant). Le masque est posé sur l'image elle-même, il reste donc en place
quand vous tournez ou recadrez. Le fichier enregistré reçoit les mêmes rendus ;
un • après le nom de la section indique qu'ils sont utilisés.

**↶ Annuler** et **↷ Rétablir** (Ctrl+Z, Ctrl+Y ou Ctrl+Maj+Z) reculent et
avancent dans les modifications ; un glissement de curseur ou un coup de
pinceau compte pour une étape, et l'info-bulle du bouton dit ce qu'il annule.

**✂ Recadrer** recadre sur l'image : l'image entière est affichée avec un
cadre ; faites glisser pour tracer un nouveau cadre, tirez ses bords ou ses
coins pour l'ajuster, faites glisser l'intérieur pour le déplacer. Cliquez sur
**✂ Recadrage : terminé** pour voir le résultat. Les boutons de forme et les
curseurs de recadrage de **Rotation et recadrage** règlent le même cadre.

**Préréglages** (en haut, sous l'image) : tapez un nom et **Enregistrer** pour
garder le rendu actuel : étirement, balance des blancs, lumière, courbes,
couleur, détail et les rendus du ciel et du premier plan, mais pas la
rotation, le recadrage ni la taille d'export (ils appartiennent à une image).
**Préréglage** applique un rendu enregistré à l'image ouverte (Ctrl+Z
l'annule) ; **Supprimer** retire le préréglage dont le nom est tapé. Les
préréglages sont gardés dans le dossier de réglages (`presets`).

## 17. Timelapse

*Nouveau en 0.4 (en développement).* L'onglet **Timelapse** fait une vidéo à
partir d'une séquence d'images : un paysage sous le ciel nocturne pris depuis
un trépied, le ciel tournant au-dessus d'un premier plan immobile. Un zoom lent
ou un travelling façon slider à travers l'image peut lui donner un mouvement
de caméra.

**Projets :** **⬇ Enregistrer le projet…** garde les images, les rendus, le
masque (et le cercle fisheye) et chaque réglage vidéo dans un fichier
`.tlproj` ; **⇪ Ouvrir un projet…** (Ctrl+O) restaure tout, même si le dossier
a été déplacé. Ctrl+S enregistre directement dans le projet ouvert. « Partir
du rendu de l'onglet Post-traitement » est gardé comme interrupteur : il
utilise l'image ouverte à ce moment dans Post-traitement.

**Images :** ajoutez des fichiers ou un dossier ; ils défilent dans l'ordre des
noms de fichier (numérotation de l'appareil). Cliquez sur l'un d'eux pour
l'afficher au centre ; ◀ ▶ les parcourent. Avec des images RAW, **RAW via**
choisit comment elles sont lues : **intégré (rapide)** (par défaut) est rapide
et traite toutes les images de la même façon ; **rawtherapee** applique les
retouches enregistrées dans les fichiers `.pp3` de RawTherapee à côté des
images, mais prend plusieurs secondes par image (environ dix fois plus) ;
**auto** n'utilise RawTherapee que pour les images qui ont un fichier `.pp3`.
L'onglet indique combien en ont un. RawTherapee écrit un `.pp3` pour chaque
photo ouverte dans son navigateur de fichiers, en avoir un ne signifie donc
pas que la photo a été retouchée.

**Rendu :** les mêmes curseurs de lumière et de couleur que dans
Post-traitement (exposition, contraste, hautes lumières, tons moyens, ombres,
blancs, noirs, température, teinte, saturation, vibrance), réglés séparément
pour **Toute l'image**, le **Ciel** et le **Premier plan**. Ciel et premier plan
s'ajoutent au rendu de toute l'image, seulement dans leur partie du masque ; •
marque les parties modifiées. L'image choisie montre le rendu (**Montrer le
rendu** ; désactivé, l'image est montrée telle quelle). L'anti-scintillement et
le mouvement de caméra n'apparaissent que dans la vidéo.

**Masque :** comme dans Post-traitement (chapitre 16) : **✎ Peindre**,
**◎ Détecter le ciel**, **Tout le ciel**, **Inverser**, **Vider**, **Bord doux**,
**Montrer le masque**. Un seul masque sert à toutes les images, car sur
trépied le premier plan ne bouge pas.

Pour un timelapse du ciel entier avec un fisheye circulaire pointé vers le
haut, **◯ Cercle fisheye** trouve le cercle image éclairé et fait du ciel un
disque en son milieu ; l'anneau autour (horizon, arbres, lumières) est le
premier plan. **Cercle du ciel** règle quelle part du rayon du cercle est le
ciel, et un grand **Bord doux** (jusqu'à 25 %) fait de la frontière un dégradé
radial. Corrigez le reste au pinceau, p. ex. un télescope ou une personne qui
entre dans le ciel.

**Vidéo :**

| Réglage | Effet |
|---|---|
| **Anti-scintillement** | Égalise les sauts de luminosité et de couleur entre les images (paliers d'exposition, lumière changeante, la balance des blancs de l'appareil). **Léger** compare chaque image à ses voisines (**Lissage**, en images) ; **Fort** en utilise trois fois plus. Les changements lents comme le crépuscule restent. **La couleur aussi** égalise chaque couleur à part ; **Par zones** le fait sur une grille grossière dans l'image, les changements à grande échelle dans une partie (pollution lumineuse, une lumière qui passe, l'aube d'un côté) sont donc égalisés là où ils se produisent. Les deux fonctionnent avec les objectifs ordinaires et les fisheyes. La mesure lit chaque image une fois de plus. |
| **Partir du rendu de l'onglet Post-traitement** | Chaque image reçoit d'abord les réglages de l'image ouverte dans Post-traitement (étirement, balance des blancs, lumière, courbes, couleur, détail ; pas sa rotation ni son recadrage), puis le rendu réglé ici. Les valeurs mesurées par l'étirement automatique viennent de la première image et servent à toutes, la vidéo ne scintille donc pas. |
| **Travelling (slider)** | Comme si l'appareil roulait sur un slider motorisé : la vidéo montre une partie de l'image plus petite de ce pourcentage (jusqu'à 20 %) et la déplace à travers l'image, **Gauche → droite**, **Droite → gauche**, **Haut** ou **Bas**. Le mouvement est fluide, pas par pas de pixel entier. |
| **Zoom lent** | Zoome lentement sur toute la vidéo ; peut se combiner avec le travelling. |
| **Démarrer et s'arrêter en douceur** | Le mouvement et le zoom accélèrent au début et ralentissent à la fin ; désactivé : même vitesse tout du long. L'aperçu indique où la vidéo commence (clair) et finit (pâle). |

**Sortie :**

| Réglage | Effet |
|---|---|
| **Format** | **MP4** (H.264) et **WebM** (VP9) sont faits avec ffmpeg. Installez-le (ffmpeg.org ; Windows : `winget install ffmpeg` ; Linux : le gestionnaire de paquets ; macOS : Homebrew) ou placez-le à côté du programme ; l'onglet indique où il l'a trouvé. **GIF animé** et **Images numérotées** (TIFF ou PNG 16 bits, JPEG) ne demandent rien d'autre. Les GIF de champs d'étoiles sont lourds (environ 1 Mo par image en 720p ; le MP4 est 5 à 10 fois plus petit), ils conviennent donc aux courts extraits. |
| **Images par seconde** | 12–60 images par seconde. |
| **Résolution** | Comme les images, 4K, 1440p, 1080p ou 720p ; jamais agrandi. Jusqu'en 1080p, les RAW sont lus en demi-taille, ce qui est plus rapide et encore assez net. |
| **Forme** | La forme de la vidéo ; l'image y est recadrée et l'aperçu montre le recadrage. **Comme les images** (pas de recadrage), **16:9**, **3:2** et **4:3** (ils suivent l'orientation des images), **1:1 carré** (tout le cercle d'un fisheye ciel entier) ou **9:16 vertical** (téléphones). En 1080p : 1920 × 1080, 1620 × 1080, 1440 × 1080, 1080 × 1080 et 1080 × 1920. Avec un cercle fisheye, l'onglet avertit quand une forme coupe une partie du ciel. |
| **Qualité** | Pour MP4, WebM et les images JPEG : plus haut = meilleure image, fichier plus gros. |
| **Lire en avant, puis en arrière** | Les images à nouveau à l'envers à la fin, la vidéo boucle donc sans à-coup. |
| **Garder la dernière image** | Garde la dernière image affichée quelques secondes. |

Sous les réglages : combien d'images, la durée de la vidéo et sa taille en
pixels. **▶ Créer la vidéo** travaille en arrière-plan, jusqu'à huit images à
la fois (moins quand la mémoire manque), avec une barre de progression (temps
écoulé et restant) et **✖ Annuler** (rien n'est conservé). Un rapport montre
ensuite les images, la taille de la vidéo, la cadence et la durée, le temps
mis et le fichier avec sa taille, ou ce qui n'a pas marché. Par exemple, 69
images RAW (24 Mpx) sont devenues un MP4 1080p avec anti-scintillement en
environ une demi-minute sur un PC de bureau.
