# Oscillations de contact — 14 octobre 2026

## Diagnostic et correction

Le jitter signalé aux épaules existe dans les positions acceptées par le solveur. Sur un T-shirt déjà drapé, multiplier les itérations de 16 à 64 ne ramenait le déplacement haute fréquence P95 que de 0,701 à 0,564 mm. Une pièce horizontale, initialement 50 µm au-dessus de la zone de contact d'un plan, reproduisait des bonds normaux de 1,35 mm. Aucun rendu n'intervient dans ces reproductions.

La détection n'incluait que les interactions déjà à une distance inférieure au rayon `r`. Une prédiction ou une itération pouvait entrer dans cette zone pendant le pas, sans rencontrer le potentiel correspondant. Le pas suivant réintroduisait une forte répulsion ; ses corrections devenaient de la vitesse. Les bornes OGC pouvaient empêcher une traversée sans empêcher cette oscillation de la couche d'offset.

`ogc/world.rs` conserve désormais les interactions admissibles proches jusqu'au rayon de recherche `rq`, y compris celles dont l'énergie vaut encore zéro. **L'activation physique reste à `r = 3 mm`** : ni force préventive à distance, ni nouvelle épaisseur, ni amortissement supplémentaire. La coloration inclut aussi ces termes dormants. Le potentiel et ses dérivées sont réévalués dans le solve et sa recherche locale ; centres, distances et rayons conservatifs restent ceux de la détection. Le tri et la déduplication des identifiants canoniques remplacent les insertions dans un arbre par contact ; les trajectoires du T-shirt sont identiques bit à bit avant/après cette optimisation.

Le [papier OGC](https://graphics.cs.utah.edu/research/projects/ogc/Offset_Geometric_Contact-SIGGRAPH2025.pdf), algorithmes 1–3 et §4.3, a été relu : sa sélection à `r` s'accompagne d'un renouvellement des ensembles pendant la résolution. Notre backend hybride n'effectue qu'une détection par pas. La conservation des termes jusqu'à `rq` est une adaptation indépendante à cet ordonnancement, pas une reproduction littérale de ces algorithmes. Les types de primitives et régions admissibles restent choisis lors de la détection ; cette extension ne garantit pas une couverture complète de toutes les activations lors de grands mouvements ou changements de feature.

Le calcul GPU travaille maintenant dans un repère translaté au centre du vêtement, calculé en f64 à chaque pas. Positions, cibles, bornes et saisie souris utilisent ce même repère ; la lecture rétablit les coordonnées mondiales. Membrane et coutures utilisent des différences de positions pour limiter les annulations en f32. Les dérivées utilisent les mêmes coefficients que ces expressions. La géométrie de repos, les poids matériels, la gravité, l'initialisation adaptative et les 16 itérations restent inchangés. Ce n'est pas un filtre d'affichage.

`VbdMetrics.contacts` compte les termes d'énergie positive à la détection ; `contact_terms` compte toutes les interactions retenues. `ContactSet.contacts` comprend maintenant les termes dormants. Ne pas interpréter ce nombre comme un nombre de collisions actives. Le coût supplémentaire des termes dormants n'est pas gratuit ; le noyau GPU quitte leur évaluation avant les dérivées quand la distance dépasse `r`.

## Mesures

RTX 3090 / Vulkan, deux threads CPU, dt 1/60 s, 16 itérations, mêmes snapshots de départ dans chaque comparaison. Traces de trois secondes ; première seconde exclue des statistiques. Sélection fixe des sommets dont la hauteur initiale dépasse 1,35 m. Mesure : `norm(x[t] - (x[t-1] + x[t+1])/2)` ; elle retire le déplacement à vitesse constante, mais peut aussi mesurer une vibration physique. Les résultats complets et empreintes des états initiaux sont dans [jitter-validation.json](jitter-validation.json).

| Scène | P95 avant → après | RMS avant → après | Pointe après |
|---|---:|---:|---:|
| T-shirt, 2 454 sommets | 0,701 → 0,084 mm (**−88%**) | 0,307 → 0,045 mm | 0,860 mm |
| T-shirt + tablier privé, 3 139 sommets | 0,729 → 0,111 mm (**−85%**) | 0,320 → 0,056 mm | 0,695 mm |

La pièce sur plan ne présente plus le cycle de rebond : déplacement normal maximal mesuré après chauffe inférieur à 0,001 µm. Les tests utilisent une tolérance plus large de 1 µm. La première assertion essayée sur le déplacement **3D** échouait à quelques micromètres de relaxation tangentielle ; le test de contact mesure donc explicitement la composante normale, sans prétendre figer le glissement. Une translation de toute la scène de plus de 100 m ne change pas le résultat au-delà de 2 µm. Le test de tissu suspendu conserve environ 0,22% d'extension après cinq secondes. Un T-shirt neuf après quatre secondes donne un écart maximal de couture de 2,08 mm et un allongement P95 de 3,88%.

La mesure native de cinq secondes après chauffe donne **60,01 images/s et 60,41 pas physiques/s** pour le T-shirt (1,007× ; rattrapage borné de l'horloge). Pour le tablier : **59,93 images/s et 50,18 pas physiques/s**, soit 0,836× ; **échec du seuil temps réel**. Les temps headless fluctuent avec la charge de la station ; ils ne prouvent pas un gain de débit contrôlé. Le tablier reste coûteux et quelques pointes subsistent : ni absence universelle de jitter, ni convergence complète, ni friction ne sont revendiquées.

## Vérifications et reproduction

78 tests CPU et trois tests GPU opt-in passent ; formatage, Clippy CPU/desktop et cinq templates passent. Le test natif de saisie souris passe. Les forces/énergies GPU d'un T-shirt drapé sont comparées à l'AD CPU sur deux pas : erreur absolue maximale de gradient 1,07×10⁻⁴ N. Les captures des scènes natives ont été inspectées. Les tests CPU du contact vérifient notamment l'activation au cours du même pas, l'absence de force avant `r`, les bornes originales et l'équivalence du cache avec une détection fraîche.

```powershell
.\scripts\check-cpu.ps1
cargo clippy --locked -j 2 --all-targets -- -D warnings
cargo build --release --locked -j 2 --bin cloth-fit --example motion_probe

# GPU explicite, hors suite ordinaire :
cargo test --release --locked -j 2 --test gpu_motion -- --ignored --test-threads=1 --nocapture
.\target\release\examples\motion_probe.exe --project chemin/vers/fitting-project.json --output output/motion --gpu --steps 180 --iterations 16
python scripts/analyze-motion.py output/motion --min-y 1.35 --warmup 60
.\target\release\cloth-fit.exe --project output/motion/fitting-project.json --steps 0 --ui-perf-smoke --output output/motion-ui
```

`motion_probe` est CPU par défaut, écrit les états acceptés et leurs métriques avec buffers, sans modifier le projet source. Ces traces sont des diagnostics volumineux, pas un nouveau format d'édition. Les écritures sont hors de `step_ms` ; ce programme ne mesure pas la cadence d'une fenêtre. Le script Python n'utilise que la bibliothèque standard. Pour comparer deux versions, garder le même fichier de départ, vérifier l'empreinte d'état et les réglages, puis analyser les mêmes sommets. La référence avant correction est `f385ed0`, instrumentée avec ce même exemple ; les snapshots privés et les traces ne sont pas distribués dans Git.
