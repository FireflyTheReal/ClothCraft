# Offset Geometric Contact et VBD

Implémentation Rust indépendante du contact normal décrit par Chen et al., **Offset Geometric Contact**, SIGGRAPH 2025. Source primaire : [projet et article](https://graphics.cs.utah.edu/research/projects/ogc/). Le PDF a été lu, notamment les sections 3.2–3.7 et les algorithmes 1–4. Le [tutoriel officiel IPC Toolkit](https://github.com/ipc-sim/ipc-toolkit/blob/main/docs/source/tutorials/ogc.rst) a aussi été consulté ; aucun code C++ n'a été incorporé.

## Ce qui est implémenté

- `ogc/topology.rs` : adjacences et identifiants canoniques des faces, arêtes et sommets. Arêtes manifold, contours ouverts autorisés.
- `ogc/geometry.rs` : régions admissibles des sommets et arêtes (équations 8–10 et 14–15), plus proches primitives, diagnostic d'intersection initiale.
- `ogc/bvh.rs` : racines séparées tissu/mannequin, refit des primitives mobiles et filtrage des boîtes individuelles dans les feuilles. Requêtes sans limite silencieuse du nombre de candidats.
- `ogc/candidates.rs` : cache conservatif des identifiants de paires. Recherche à `R + 2s + epsilon`, avec `R >= rq` arrondi vers le haut et `s = rayon OGC`. Réutilisation seulement si chaque sommet mobile reste à moins de `s` de son centre de cache et si `rq <= R`. Par convexité et inégalité triangulaire, une distance entre primitives varie au plus de `2s`. Les distances, régions admissibles, forces et bornes sont **recalculées à chaque détection**. La géométrie fixe ne peut pas changer sans reconstruction. Deux tâches CPU indépendantes fusionnent les minima et les contacts canoniques.
- `ogc/world.rs` : VF dans les **deux sens**, EE, auto-contact, déduplication des contacts sur une même primitive, distances conservatives propagées aux sommets de chaque primitive. Les distances pour les bornes sont calculées même lorsque la région admissible ou le filtre des arêtes parallèles rejette une force.
- `ogc/potential.rs` : activation quadratique puis logarithmique, dérivées exactes par AD, filtre polynomial des arêtes presque parallèles. Une interaction primitive canonique est intégrée une seule fois ; les contributions VV/VE communes aux parcours VF et EE sont fusionnées.
- Bornes sphériques `0.45 * min(dVF, dEE, dFV)` autour des positions à la **dernière détection**, et non autour des positions à la dernière itération. Une projection prévue par OGC limite chaque proposition locale avant son contrôle énergétique.
- `vbd.rs` : Euler implicite, même membrane anisotrope, flexion et coutures que la référence. Newton local 3×3, Hessienne exacte régularisée SPD, recherche locale décroissante. Coloration comprenant aussi les contacts et l'attachement souris : aucun élément partagé dans une couleur.
- `gpu_vbd.rs`, `vbd.wgsl` : calcul des énergies, forces et itérations VBD sur GPU, Hessiennes locales positives approchées. Le gradient conserve les dérivées du filtre EE. Le mode `--check-gpu-gradients` compare énergies/forces GPU à l'AD f64 à géométrie identique arrondie en f32.

## Prédiction et candidats conservatifs

**Correction du jitter :** les interactions géométriquement admissibles sont maintenant conservées jusqu'à `rq`, y compris quand leur potentiel est nul à la détection. Seul `r` active une force. Cette adaptation à la détection unique par pas permet à une interaction de devenir active pendant le solve, sans attendre le pas suivant. Elle ne garantit pas une couverture exhaustive après un grand mouvement ou un changement de feature. Coloration et recherche locale incluent les termes dormants ; les bornes ne changent pas. Un tri/dédoublonnage canonique remplace les insertions dans un arbre. Le GPU utilise des coordonnées translatées près du vêtement et des différences de positions. Voir [diagnostic, mesures et limites](jitter.md) ; `contacts` compte les interactions d'énergie positive à la détection, `contact_terms` toutes les interactions retenues.

La construction des candidats parcourt maintenant les arbres par **paires de nœuds** : points/faces pour VF, arêtes/arêtes pour EE. Un BVH de points possède lui aussi des racines mobiles/fixes ; seules les primitives mobiles sont refittées. Les paires fixes/fixes et les paires de boîtes éloignées sont rejetées par groupes ; les VF réciproques et les EE restent présents. Le parcours EE visite chaque paire non ordonnée une fois et conserve l'ordre historique des opérandes pour les calculs géométriques. Les tests comparent les ensembles aux recherches exhaustives après déformation. Voir [recherche et choix de performance](performance-research.md).

Le CPU et le GPU commencent le pas par une **initialisation adaptative VBD (§3.7)**, tronquée aux **bornes de la détection initiale**. La prédiction collective conserve la chute libre ; l'accélération estimée du dernier pas accepté évite de réinjecter artificiellement une chute libre dans les points déjà soutenus. La cible inertielle de l'objectif conserve toute la gravité. L'estimation tient compte de l'amortissement et son historique est invalidé après édition/rechargement. Elle respecte les sommets fixés et les marges f32 ; les centres ne sont pas déplacés entre prédiction et solve. Voir [diagnostic et correction de l'étirement](adaptive-initialization.md). Le test opt-in `tests/gpu_motion.rs` couvre la chute libre, la croissance des buffers, les bornes sous forte vitesse et le tissu suspendu sur plusieurs secondes.

La construction des candidats utilise une borne inférieure de distance entre AABB ; le calcul exact VF/EE reste dans la détection à chaque pas, y compris le premier. Cela peut retenir des faux positifs mais ne doit retirer aucune paire proche. `tests/ogc_bvh.rs` compare cette borne aux distances géométriques et `tests/ogc_cache.rs` compare les résultats mis en cache à une reconstruction fraîche.

## Coefficient du raccord logarithmique

Le PDF affiche à l'équation 19 `k' = tau * k * (tau-r)^2`. Ce coefficient ne satisfait pas la continuité de la dérivée annoncée : `g'_quadratique(tau) = k*(tau-r)`, `g'_log(tau) = -k'/tau`. Le code utilise donc le coefficient **dérivé des conditions de raccord** `k' = k*tau*(r-tau)`. Avec `tau=r/2`, les deux dérivées secondes valent `k`. Le logarithme porte sur le rapport sans dimension `d/tau`. Des tests de différences finies et des deux côtés du raccord vérifient ce choix. À `d=r`, la loi est C1, pas C2 ; ne pas annoncer C2 partout.

## Contrat et limites

OGC empêche de créer une intersection à partir d'un état séparé, pour les paires incluses dans la détection et sous les hypothèses géométriques de l'article. Il ne répare pas un état déjà traversé. Le constructeur refuse les intersections initiales détectées ; les erreurs de pas sont transactionnelles. Le diagnostic initial utilise des tolérances flottantes : ce n'est pas un prédicat géométrique exact certifié.

Les coutures conservent deux bords distincts. Les sommets reliés par une même quadrature de couture sont donc exclus **localement** des collisions correspondantes. Les panneaux entiers restent en auto-contact. La garantie ne couvre pas les paires exclues. Une topologie cousue ou un traitement d'épaisseur dédié reste nécessaire pour des coutures industrielles.

Les contacts normaux sont doubles faces, y compris sur le mannequin. Nous n'utilisons pas la variante volumique unilatérale du §3.9. Pas de friction de Coulomb, avatar animé, restitution de collisions déjà présentes ni fils physiques. Les paramètres de tissu ne sont pas calibrés.

Le rayon d'offset (m) est distinct de l'épaisseur matérielle. `stiffness` est un coefficient **N/m par paire**, pas la pression N/m³ de l'ancien collider. Son effet dépend donc du maillage et de la multiplicité des contacts ; une convergence en résolution n'est pas revendiquée.

Les ensembles de contacts sont figés entre deux détections. Le CPU les renouvelle lorsque suffisamment de sommets atteignent leur borne. Le backend hybride renouvelle la détection **une fois par pas**, et conserve les mêmes centres/rayons durant toutes les itérations GPU. Cela préserve les bornes mais peut ralentir le glissement et la relaxation. Le GPU soustrait une marge d'arrondi aux rayons ; une séparation inférieure à 2 µm interrompt le pas. Ce n'est pas une implémentation entièrement résidente sur GPU : BVH, détection, coloration, empaquetage et lecture des résultats restent CPU. Les allocations GPU, bind groups, readback et queries sont réutilisés et agrandis au besoin ; les données dynamiques sont toujours transférées par pas. Les termes matériels et leur incidence sont compilés une fois par vêtement, avec invalidation par identité Arc conservée. Un workgroup de 32 threads assemble les contributions de chaque sommet ; les réductions sont synchronisées et la coloration reste inchangée. La somme parallèle modifie les arrondis f32 : les trajectoires ne sont pas bit à bit identiques à l’ancien noyau série.

Un budget fixe de VBD ne garantit pas la convergence du minimiseur à chaque pas. Le résidu CPU est indiqué ; le résidu GPU non calculé est `null`. `step_ms` est un temps mural complet. Sur GPU, `detection_ms` inclut détection et coloration ; `solve_ms` inclut préparation, transferts et attente GPU. Aucun de ces nombres n'est un timestamp GPU pur.

Le champ séparé `gpu_compute_ms`, lorsqu'il est disponible, est mesuré par timestamps matériels de la passe compute. Il exclut la détection CPU et les transferts. Réglages de la passe retenue : rayon 3 mm, raideur de contact 100 000 N/m par paire, coutures de patron 50 000 N/m², 8 itérations, pas de 1/60 s. Ces choix ne constituent pas une calibration matérielle.

Le test `--ui-interaction-smoke` injecte des événements de pointeur dans une vraie fenêtre egui, vérifie qu'un triangle a été saisi et qu'un pas avec cet attachement a été accepté, écrit `interaction.json`, capture puis ferme la fenêtre. Il nécessite explicitement le GPU et reste hors des tests ordinaires.

## Temps réel

La passe [Turbo](turbo-perf.md) mesure séparément le débit physique et la vraie fenêtre. `atelier/worker.rs` possède l’état courant et une horloge à pas fixe, indépendante du dessin. Rattrapage borné à quatre pas après une pointe de durée, sans augmenter dt ni réduire les itérations. Les contrôles les plus récents remplacent les anciens ; un seul résultat attend l’interface, et les générations empêchent un ancien maillage de remplacer le nouveau. `--ui-perf-smoke` mesure cinq secondes après une seconde de chauffe, avec le compteur de frames egui unique, et écrit `ui-perf.json`.

## Exécuter

```powershell
# Référence OGC/VBD sans GPU
cargo run --release --locked --no-default-features -j 2 --bin cloth-fit -- --steps 30 --mesh-mm 30 --output output/ogc-cpu

# Calcul GPU explicitement demandé, mesures complètes
cargo run --release --locked -j 2 --bin cloth-fit -- --gpu-sim --steps 120 --mesh-mm 30 --output output/ogc-gpu

# Validation numérique GPU (coûteuse, opt-in)
cargo run --release --locked -j 2 --bin cloth-fit -- --gpu-sim --check-gpu-gradients --steps 4 --mesh-mm 50 --output output/ogc-check

# Atelier : calcul asynchrone, saisir le tissu au bouton gauche
cargo run --release --locked -j 2 --bin cloth-fit -- --steps 0 --view --output output/atelier
```

`--reference` conserve le Newton/PCG historique avec sa pénalité de contact discrète. Ne pas le confondre avec OGC. `--gpu-benchmark` mesure toujours le **rendu** ; `--gpu-sim` exécute le calcul de physique hybride. Les tests ordinaires restent entièrement CPU ; ils valident aussi le WGSL via Naga.
