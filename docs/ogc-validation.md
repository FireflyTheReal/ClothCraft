# Validation OGC / VBD — 14 octobre 2026

**Archive de la première implémentation.** La passe [Turbo](turbo-perf.md) décrit le code et les performances actuels. Les chiffres ci-dessous restent ceux mesurés avant optimisation.

Mesures locales réellement exécutées sur **NVIDIA GeForce RTX 3090, Vulkan, pilote 616.56**, build Rust release, deux threads CPU. Les résultats bruts légers sont dans [ogc-validation.json](ogc-validation.json) ; snapshots, journaux et captures locales sous `output/ogc-validated/`.

## Essais de vêtements

Deux panneaux plans initialement séparés de 530 mm, mannequin statique de 26 756 triangles, Delaunay cible 30 mm. Membrane/flexion de référence non calibrées. OGC normal : rayon 3 mm, raideur 100 000 N/m par paire, relaxation 0,45. Coutures : 50 000 N/m². 360 pas de 1/60 s, 8 itérations VBD, soit **6 s simulées**.

| Mesure | T-shirt | Robe |
|---|---:|---:|
| Sommets / triangles de tissu | 1 744 / 3 212 | 2 808 / 5 324 |
| Temps mural des 360 pas | 17,79 s | 20,87 s |
| Moyenne complète par pas | 49,43 ms | 57,98 ms |
| Médiane des 60 derniers pas | 53,10 ms | 55,98 ms |
| P95 des 60 derniers pas | 67,43 ms | 79,82 ms |
| Détection + coloration CPU, médiane finale | 34,96 ms | 37,13 ms |
| Compute GPU seul, médiane finale | 14,83 ms | 15,05 ms |
| Ouverture maximale de couture finale | 2,78 mm | 2,53 mm |
| Distance minimale à la dernière détection | 1,66 mm | 2,71 mm |

**Le backend hybride ne tient pas encore 60 Hz physiques.** La cadence de calcul finale correspond à environ 18–19 pas/s, pour des pas physiques de 1/60 s. L'interface est asynchrone ; cela ne transforme pas ce débit en simulation temps réel. Les temps varient avec la scène, les contacts et l'activité de la machine. Les médianes et P95 portent uniquement sur la fin de ces trajectoires, pas sur toutes les scènes possibles.

Le maximum de couture est surtout aux jonctions sous les bras sur le T-shirt ; les épaules sont à environ 0,37–0,38 mm. La robe reste sous 2,54 mm. Ce sont des pénalités finies, pas des coutures exactement fermées. Les snapshots finaux ont été rouverts et ont passé le diagnostic initial d'intersections, dans le périmètre des paires non exclues. Ce diagnostic flottant n'est pas une certification géométrique exacte.

## Vérifications

- **54 tests CPU**, y compris les suites historiques. OGC : continuité de la loi, différences finies du gradient/Hessienne, action/réaction, objectivité, plusieurs couches de contact, déduplication, VF réciproque et EE, cumul des bornes, état initial intersecté, ordre des primitives statiques/dynamiques et régression sur un sommet partagé presque coplanaire.
- VBD : chute libre analytique, repos conservé par translation, force de saisie et échec transactionnel. Les deux shaders sont analysés et validés par Naga, sans GPU.
- Comparaison de toutes les forces locales GPU avec l'AD f64 sur les snapshots en contact : erreur absolue maximale **1,084e-4 N** sur le T-shirt et **1,434e-3 N** sur la robe. Énergies également contrôlées. Comparaison à géométrie arrondie identique ; tolérance de force 3 mN + 0,3 % de la norme. Les Hessiennes GPU sont des approximations positives, pas les Hessiennes exactes CPU.
- Test de saisie dans une **vraie fenêtre egui** : événements de pointeur, triangle sélectionné, pas GPU avec attachement accepté, positions déplacées de 1,35 mm au maximum. Rapport `interaction/interaction.json` : `passed=true`. Cela vérifie le chemin événement → picking → worker → solveur → affichage, pas une étude ergonomique complète.
- Clippy CPU/desktop sans avertissement et formatage vérifiés. Aucune exécution GitHub CI revendiquée.

Le contrôle de réouverture a exposé un faux positif sur deux triangles ayant un sommet commun : un déterminant très faible produisait un temps d'intersection presque égal à 1. Le diagnostic ignore maintenant les segments ayant un **endpoint topologiquement partagé** avec le triangle testé ; leurs arêtes opposées restent vérifiées, et les recouvrements coplanaires passent aussi par les distances VF/EE. Un test reproduit les coordonnées concernées.

## Rendu et captures

Les vrais ateliers T-shirt et robe ont été ouverts et capturés. Bibliothèque, vue 3D, patrons 2D et propriétés sont séparés ; le vêtement et le mannequin utilisent des normales interpolées. La densité géométrique n'est pas modifiée par ce lissage d'affichage.

À 1 200 × 1 200, 12 frames de chauffe puis 60 mesures : préparation/soumission/attente du rendu médiane **1,28 ms** T-shirt et **1,09 ms** robe ; passe de rendu GPU seule **0,018–0,020 ms**. Ce sont des rendus statiques hors écran : ni mesure du solveur, ni preuve de FPS de la fenêtre. Les caches du mannequin et les transformations caméra GPU évitent sa reconstruction par frame.

## Reproduire

```powershell
.\scripts\check-cpu.ps1
.\scripts\ogc-benchmark.ps1 -Gpu -UiSmoke -Steps 360 -MeshMm 30

.\target\release\cloth-fit.exe --project output/ogc-benchmark/tshirt/fitting-project.json --gpu-sim --check-gpu-gradients --steps 1 --output output/force-check

.\target\release\cloth-fit.exe --project output/ogc-benchmark/tshirt/fitting-project.json --steps 0 --ui-interaction-smoke --output output/interaction-check
```

Les sorties de vérification des gradients incluent du calcul AD CPU supplémentaire et ne doivent pas être utilisées comme benchmark. La commande de saisie ouvre une fenêtre et se ferme automatiquement. Les captures seules ne démontrent ni convergence ni absence globale d'intersection.

## Suite technique

Priorité : détection VF/EE et refit BVH GPU, buffers de solve résidents et meilleure occupation du GPU par les petits groupes de couleurs. Conserver l'implémentation f64 comme oracle. Ensuite : renouvellement des bornes durant un pas GPU, friction, fermeture des coutures par contraintes plus fortes et traitement explicite de l'épaisseur. Voir [contrats et différences avec l'article](ogc.md).
