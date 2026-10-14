# Premier essayage : mannequin, T-shirt et robe

**Archive de la passe Newton antérieure à OGC.** Le script emploie désormais explicitement `--reference`. Les nouveaux patrons sont plus larges ; ne pas comparer leurs résultats aux anciennes mesures comme s'il s'agissait de la même scène. L'état courant, le calcul GPU et les interactions de l'atelier sont décrits dans [OGC](ogc.md) et [validation OGC](ogc-validation.md).

Le binaire `cloth-fit` charge un mannequin statique et deux patrons indépendants de test : un T-shirt à manches intégrées (8 raccords) et une robe sans manches évasée (4 raccords). Col, poignets/emmanchures et ourlet restent ouverts. Ils n'ont pas de marges de couture ni de gradation industrielle.

## Reproduire

```powershell
# Maillage, solveur et exports CPU seulement
.\scripts\first-fit.ps1 -Steps 5 -MeshMm 25

# Opt-in GPU : rendu mesuré et capture de la vraie fenêtre native
.\scripts\first-fit.ps1 -Steps 5 -MeshMm 25 -Gpu -UiSmoke

# Explorer le résultat, fenêtre initialement en pause
cargo run --locked -j 2 --bin cloth-fit -- --project output/first-fit/tshirt/fitting-project.json --steps 0 --view --output output/viewer
```

Les boutons **T-shirt / Robe** chargent les scènes de `output/first-fit/` lorsqu'elles existent, sinon les presets. **Taille cible**, **Anisotropie**, **Axe** et **Re-mailler / Reset** contrôlent la [Delaunay contrainte](meshing.md). Un remeshing repart de l'arrangement initial ; il ne transfère pas l'ancien drapé sur la nouvelle topologie. Le rapport géométrique est dans `run.json` pour chaque panneau.

`--mesh-mm 15` permet un essai plus dense ; `--steps 0` prépare et exporte sans avancer la physique. `--gpu-benchmark` réalise 12 images de chauffe et 60 images mesurées en 1200×1200. `--ui-smoke` ouvre, capture puis ferme la fenêtre. Aucun de ces chemins GPU n'est exécuté par les tests ordinaires.

## Modèle et portée des résultats

La physique est toujours **CPU f64**, Newton/PCG avec coutures intégrées. Le mode par défaut est une **relaxation incrémentale d'assemblage** : chaque pas part avec une vitesse nulle, et le temps enregistré est un pseudo-temps. Quelques pas convergés ne signifient ni un équilibre du vêtement ni une simulation dynamique validée. `quasistatic: false` permet le mode Euler implicite dynamique dans un projet.

Le prépositionnement rapproche les bords en profondeur et éloigne les sommets de la silhouette du corps avant la résolution. Il ne modifie pas le patron de repos. Il peut introduire de l'étirement et des intersections de faces. La fermeture des coutures peut donc d'abord augmenter leur écart sous l'effet de la relaxation du tissu.

Le corps utilise une BVH CPU statique et une distance locale signée au triangle le plus proche. Le contact actuel est une pénalité **aux sommets** : pas de CCD, friction, contacts face/face ou auto-collision. Le signe local n'est pas un oracle global d'intérieur pour tout OBJ. Une triangulation plus dense améliore l'échantillonnage mais ne garantit pas l'absence d'intersection. `contact_clearance` peut ajouter une enveloppe numérique (0 par défaut) ; la métrique de pénétration mesure alors la violation de cette enveloppe.

Un pas rejeté restaure intégralement l'état précédent. Au plus sept essais divisent le pas par deux ; les pas suivants conservent la réduction. `run.json` indique les pas demandés/acceptés, le statut, l'erreur éventuelle, le temps mural comprenant les essais rejetés et la moyenne des seuls solves acceptés. `metrics.jsonl` conserve les résidus, la tolérance, dt, les écarts de couture et les pénétrations. Même un échec exporte le dernier état accepté et renvoie un code non nul.

Le benchmark GPU mesure **le rendu statique**, pas la physique : timestamps GPU du pass 3D, et séparément préparation CPU + soumission + attente. Il exclut la présentation de fenêtre et ne constitue pas une mesure de FPS interactifs. Le script utilise un build CPU release pour le solveur et un build desktop dev optimisé pour le rendu. `gpu-render.json`, `preview.png`, `front.png`, `back.png` et `ui.png` sont dans le sous-dossier `gpu/` ; les mesures CPU restent à la racine de chaque cas.

## Résultats du 14 octobre 2026

Exécution locale, deux threads CPU, RTX 3090 / Vulkan, pilote NVIDIA 616.56. Valeurs complètes : [first-fit-results.json](first-fit-results.json).

| Cas, cible 25 mm | Triangles | Sommets | Angle minimum | Pas acceptés | Couture max après 5 pas | Pénétration sommet max |
|---|---:|---:|---:|---:|---:|---:|
| T-shirt | 3 140 | 1 704 | 25,08° | 5/5 | 30,43 mm | 5,05 mm |
| Robe | 5 664 | 3 000 | 25,31° | 5/5 | 47,74 mm | 5,97 mm |

Le solveur résout les objectifs des pas acceptés, mais les vêtements ne sont **pas stabilisés ni exempts d'intersections**. Les captures après cinq pas montrent encore des zones traversées par le corps. Le maillage précédent ne comptait que 384 triangles pour le T-shirt et 256 pour la robe.

Les durées moyennes des solves acceptés sont respectivement 3,19 et 4,05 s ; les durées murales des cinq pas, essais rejetés compris, sont 36,22 et 87,70 s. Ces petites scènes ne tournent donc pas en temps réel avec cette référence CPU.

Le pass GPU de rendu mesure une médiane de 0,0184 ms et un p95 de 0,0195 ms dans les deux cas ; préparation CPU, soumission et attente prennent respectivement 6,95 et 7,72 ms en médiane. Ces temps ne mesurent pas le solveur et ne prédisent pas ses performances sur GPU.

Une passe **sans simulation** à 15 mm produit 8 768 triangles et 4 612 sommets pour le T-shirt, angle minimum 25,16°. Une passe anisotrope 2:1 à 90° et 25 mm produit 1 596 triangles, angle métrique minimum 25,54° (angle euclidien 13,44°). Les captures de ces deux cas vérifient la densité et l'orientation du maillage ; elles représentent l'arrangement initial.

## Formats et modules

- `GarmentTemplate` v2 ajoute `polygon` (points, noms de segments, réglages de maillage), `pin_points` matériels et `BoundaryRef.edge`. `pattern.width/height` reste la boîte de présentation. Les IDs et les intervalles sémantiques de couture sont préservés lors du raffinement.
- `FittingProject` v1 contient template, chemin d'avatar, contact, solveur, mode et cache d'état facultatif. Un chemin relatif d'avatar est résolu contre le fichier JSON. Les sauvegardes de session utilisent un chemin absolu ; pour partager, préférer les templates sans cache et l'avatar du dépôt.
- `avatar.rs` : import OBJ, surface et BVH ; `fitting.rs` : arrangement et transaction des pas ; `garment/meshing.rs` : Delaunay ; `fitting_editor.rs` : interface ; `gpu_bench.rs` et `render.rs` : affichage/mesures.

## Provenance du mannequin

Base mesh officiel MakeHuman, révision `a8bc2d54ff0ac92e78ff71431b1023eda42bf482`, téléchargé le 14 octobre 2026. La [licence officielle distingue le programme et les assets](https://raw.githubusercontent.com/makehumancommunity/makehuman/a8bc2d54ff0ac92e78ff71431b1023eda42bf482/LICENSE.md) et place le base mesh sous CC0. Aucun code du programme MakeHuman n'a été incorporé.

`assets/makehuman/` contient source OBJ, dérivé, licence et provenance. `python scripts/prepare-avatar.py` extrait uniquement le groupe `body`, retire les helpers, triangule les quads et normalise la hauteur à 1,75 m, pieds au niveau du sol. Résultat : **13 380 sommets, 26 756 triangles**. `measurements.json` conserve la transformation, les repères et le SHA-256 de la source.

La licence des sources propres au projet reste à choisir par le propriétaire avant publication. Le dépôt n'a pas été publié.
