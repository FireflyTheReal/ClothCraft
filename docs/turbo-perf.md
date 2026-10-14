# Turbo performance — 14 octobre 2026

**Mesures historiques antérieures aux corrections de mouvement GPU.** La [validation du tablier](apron-validation.md) révèle une prédiction inertielle manquante ; la [passe suivante sur la fausse élasticité](adaptive-initialization.md) remplace ensuite la prédiction systématique de chute libre par une initialisation adaptative. Le défaut actuel est 16 itérations. Les résultats ci-dessous restent utiles pour comparer les anciennes versions, sans établir les performances ou la qualité actuelles.

**Temps réel mesuré dans l'atelier sur les deux vêtements assemblés**, sur RTX 3090 / Vulkan, avec deux threads CPU. Le maillage, les huit itérations, le pas physique de 1/60 s et les lois matérielles/contact n'ont pas été réduits pour obtenir le gain. La détection reste CPU ; ce n'est pas un port OGC entièrement GPU.

## Comparaison à réglages identiques

Ancien exécutable conservé avant modification, puis nouvel exécutable : chacun repart des mêmes snapshots `output/ogc-validated/{tshirt,dress}/fitting-project.json`, à 6 s simulées. 120 pas supplémentaires, maillage cible 30 mm, mêmes paramètres. Les moyennes incluent tous les pas, dont le premier, et l'écriture des métriques. P95 par rang supérieur ; aucune compilation concurrente pendant cette comparaison.

| Mesure | T-shirt avant → après | Robe avant → après |
|---|---:|---:|
| Sommets / triangles | 1 744 / 3 212 | 2 808 / 5 324 |
| Temps moyen complet par pas | **50,57 → 6,18 ms** | **56,91 → 5,35 ms** |
| Accélération | **×8,18** | **×10,64** |
| Médiane | 48,98 → 4,21 ms | 54,66 → 4,31 ms |
| P95 | 59,56 → 14,17 ms | 69,78 → 5,31 ms |
| Maximum après | 17,37 ms | 16,32 ms |
| Détection + coloration, médiane | 32,76 → 1,76 ms | 37,66 → 1,47 ms |
| Timestamp compute GPU, médiane | 13,21 → 1,11 ms | 13,50 → 1,07 ms |
| Ouverture maximale finale des coutures | 2,440 → 2,398 mm | 2,265 → 2,261 mm |

Le débit de calcul moyen correspond à environ 162 et 187 pas/s ; ce n'est pas le nombre d'images affichées. Les fréquences GPU ne sont pas verrouillées. Ces mesures locales ne sont pas une garantie universelle.

## Vraie fenêtre et maillage par défaut

Reproduction complète avec `scripts/turbo-perf.ps1 -Steps 360 -MeshMm 25 -UiPerf` : assemblage depuis les panneaux séparés, puis ouverture et mesure de l'atelier. Huit itérations conservées. Une seconde de chauffe puis cinq secondes mesurées, compteur de frames **unique** d'egui, temps physique livré par le worker. Ce compteur mesure la cadence de la fenêtre, pas une sonde matérielle du balayage de l'écran.

| Mesure à 25 mm | T-shirt | Robe |
|---|---:|---:|
| Sommets / triangles | 2 454 / 4 570 | 4 094 / 7 800 |
| Assemblage : 6 s simulées en | 4,445 s | 4,309 s |
| Moyenne complète de l'assemblage | 12,35 ms/pas | 11,97 ms/pas |
| P95 pendant l'assemblage | 26,08 ms | 24,78 ms |
| Cadence de la fenêtre, vêtement assemblé | **60,02 images/s** | **60,01 images/s** |
| Cadence physique mesurée | **59,82 pas/s** | **60,01 pas/s** |
| Temps simulé / temps réel | **0,997×** | **1,000×** |
| P95 de l'intervalle entre frames | 20,43 ms | 18,21 ms |

Les pointes de reconstruction pendant l'assemblage dépassent encore 16,67 ms. L'horloge rattrape les retards courts, jusqu'à quatre pas ; une surcharge prolongée ralentit la simulation, sans changer dt, les collisions ou les itérations. Le test automatisé accepte au moins 0,95× de vitesse physique et 55 frames/s ; les valeurs effectivement mesurées sont données ci-dessus. Ni gros maillages, ni avatars animés, ni tous les gestes de souris ne sont couverts par ce benchmark.

## Changements et invariants

- BVH avec racines séparées tissu/mannequin, refit mobile et filtrage des boîtes de primitives dans les feuilles. Les identifiants et exclusions locales de couture restent identiques.
- Cache conservatif des paires candidates avec enveloppe de mouvement et rayon de requête arrondi vers le haut. Les distances, forces, régions admissibles et rayons OGC sont recalculés à chaque détection. Aucun contact n'est ignoré pour tenir un budget fixe. Preuve de couverture et invalidation : [OGC](ogc.md).
- Deux tâches CPU pour les recherches, fusion déterministe des minima et contacts canoniques.
- Un workgroup de 32 threads par sommet assemble les forces/Hessiennes et l'énergie des essais. Synchronisation explicite, coloration inchangée, mêmes contrôles énergétiques et de bornes. L'énergie initiale est réutilisée au lieu d'être recalculée.
- Allocations, bind groups, readback et timestamps GPU réutilisés ; termes matériels et incidence statique compilés une fois par vêtement. L'Arc conservée empêche une mutation en place ou une réutilisation d'adresse de tromper ce cache. Les transferts dynamiques restent présents.
- Worker physique cadencé indépendamment du rendu, propriétaire de son état. Les changements de contrôle ne réinjectent pas un vieux snapshot. Un seul résultat attend l'interface, et les générations isolent les changements de maillage.

## Précision et vérifications

**57 tests CPU réussis**, Clippy CPU/desktop sans avertissement, WGSL validé par Naga. Nouveaux tests : BVH comparé à la recherche exhaustive après grandes déformations, cache comparé à une détection fraîche après déplacements/changements de rayon, paire entrant dans le rayon entre deux reconstructions et rejet d'un changement du mannequin fixe.

Les forces et énergies GPU coopératives ont été comparées à l'AD f64 sur des snapshots en contact, en plusieurs pas pour exercer le cache. Erreurs maximales observées : 1,623 mN sur le T-shirt à 30 mm et 0,487 mN sur la robe à 25 mm, sous la tolérance existante de 3 mN + 0,3 % de la norme. La saisie par événements souris dans la vraie fenêtre passe également : attachement accepté, déplacement maximal de 8,07 mm. Les captures des deux fenêtres ont été produites.

La réduction parallèle change l'ordre des additions f32. Les mêmes énergies et gradients ne signifient pas une trajectoire bit à bit identique : après les 120 pas comparatifs, l'écart RMS des sommets est de 0,494 mm pour le T-shirt et 0,089 mm pour la robe ; maxima 6,03 mm et 1,19 mm. Le budget de huit itérations ne certifie toujours pas la convergence. Friction et calibration industrielle restent absentes.

## Reproduire et poursuivre

```powershell
.\scripts\check-cpu.ps1
.\scripts\turbo-perf.ps1 -Steps 360 -MeshMm 25 -UiPerf
.\target\release\cloth-fit.exe --project output/turbo-repro/dress/fitting-project.json --gpu-sim --check-gpu-gradients --steps 2 --output output/force-check
.\target\release\cloth-fit.exe --project output/turbo-repro/tshirt/fitting-project.json --steps 0 --ui-interaction-smoke --output output/drag-check
```

Les tests GPU restent opt-in. [Données compactes et empreintes des exécutables](turbo-perf.json) ; journaux détaillés sous `output/turbo/` et `output/turbo-repro/`, ignorés par Git. L'ancien exécutable local permet la comparaison historique ; aucun commit/publication n'est revendiqué.

Priorités restantes : lisser les reconstructions de candidats, transférer broad/narrow phase sur GPU pour les grandes scènes, supprimer les transferts complets par pas, étendre les scènes de validation. Conserver le noyau f64 et ses tests comme oracle.
