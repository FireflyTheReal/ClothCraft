# Recherche et décisions — 14 octobre 2026

**Mise à jour après cette revue initiale :** VBD CPU, contact normal OGC VF/EE et calcul VBD WGSL sont maintenant implémentés et exécutés sur GPU. Lire [OGC](ogc.md) et [validation](ogc-validation.md). Les formulations matérielles décrites ci-dessous restent utilisées ; les formulations « futur backend » et « aucun lancement GPU » relatent la phase initiale et ne décrivent plus l'état actuel. Aucun AVBD/PD/IPC n'a été ajouté.

## Audit initial

Le répertoire de travail initial était vide, y compris les fichiers cachés. Aucun `Cargo.toml`, code Python/Warp/XPBD, test, historique Git ou modification non commitée à reprendre. Rust 1.97.1 et Cargo 1.97.1 étaient disponibles.

Portée de ce travail : début exécutable de la Phase 1. Les recherches ci-dessous sont une première revue de pages officielles, publications et licences ; elles ne constituent pas une reproduction complète des articles ou de leurs benchmarks.

## Énergie effectivement implémentée

Toutes les positions de repos `u` proviennent du patron 2D, en mètres. Pour chaque triangle :

```
Dm = [u1-u0, u2-u0]          Ds = [x1-x0, x2-x0]
F = Ds inverse(Dm)           A = abs(det(Dm))/2
w = (cos(a), sin(a))         f = (-sin(a), cos(a))
ew = (|F w|²-1)/2            ef = (|F f|²-1)/2
g = (F w) dot (F f)
E_membrane = A [phi(ew, kw) + phi(ef, kf) + ks g²/2]
```

Le facteur d'adoucissement en compression `r` et la largeur de transition `d` entrent dans :

```
p(e) = (e + sqrt(e²+d²) - d)/2
phi(e,k) = k/2 [r e² + (1-r) p(e)²]
```

Cette énergie lisse est nulle et stationnaire au repos ; elle reste objective sous rotation 3D. `kw`, `kf`, `ks` sont en N/m. `kw` et `kf` sont les coefficients asymptotiques en tension, pas les modules tangents exacts à déformation nulle (le lissage les modifie). Pas de couplage de Poisson ajusté ni de loi mesurée. Le modèle est une base indépendante en déformations de Green, **pas une implémentation exacte de Baraff–Witkin/Kim**. Des Hessiennes indéfinies restent possibles.

Pour une arête interne de longueur de repos `l`, deux aires `A1,A2`, normales unitaires cohérentes `n1,n2` et direction transverse de repos `t` :

```
B(t) = Bw (t dot w)² + Bf (t dot f)²
E_bend = [3 l²/(A1+A2)] B(t) (1 - n1 dot n2)
```

L'implémentation évalue le facteur final par `|n1-n2|²/2` pour éviter l'annulation numérique de `1-cos(theta)` près d'un angle nul. Les dérivées restent celles de l'énergie, sans clamp.

`Bw,Bf` sont en N m. Le patron est plat, donc angle de repos nul. L'énergie est invariante aux mouvements rigides et quadratique en petit angle. Cette interpolation directionnelle n'est pas le tenseur de flexion orthotrope complet ; l'énergie en cosinus est bornée et n'interdit pas le repli complet. Ce choix devra être comparé à une formulation de coque/charnière mieux conditionnée avant calibration et benchmarks de plis.

La masse nodale est `density * sum(A/3)`. La pénalité sphère vaut `k_contact * A_vertex * min(distance-R-thickness/2,0)²/2`, avec `k_contact` en N/m³. La pondération surfacique évite de multiplier arbitrairement la raideur totale quand on raffine. Les pénétrations initiales sont comptées et entrent dans cette pénalité ; le centre exact est rejeté, sans réparation générale des intersections.

## Solveur réellement disponible

Minimisation de l'énergie incrémentale :

```
I(x) = sum_i [ m_i/(2 h²) |x_i-x_old_i-h v_old_i|²
             + gamma m_i/(2 h) |x_i-x_old_i|² - m_i gravity dot x_i ]
       + E_membrane + E_bend + E_contact
```

Newton avec gradients et Hessiennes exacts de ces énergies par différentiation automatique locale d'ordre 2. Rayon parallélise les éléments ; les réductions sont faites dans l'ordre du maillage. PCG utilise l'action des Hessiennes d'éléments et un préconditionneur diagonal. Les DOF fixés sont éliminés des directions. Une courbure négative ou une direction inadmissible provoque une régularisation diagonale croissante ; la recherche Armijo vérifie ensuite la diminution de l'énergie **non régularisée**. Le damping est dans l'objectif, pas ajouté après convergence.

Le résidu est la norme RMS des forces sur les DOF libres, comparée à `max(abs_tol, rel_tol * initial_residual)`. Les résultats limités en itérations ne sont pas déclarés convergés. Une erreur de pas conserve l'état précédent. La géométrie quasi dégénérée est refusée aux positions échantillonnées ; aucune CCD n'est sous-entendue.

## Sources, intérêt et suites

| Référence primaire consultée | Décision |
|---|---|
| [Baraff & Witkin, 1998](https://people.csail.mit.edu/bmcutler/gaudi/baraff_witkin.pdf) | Référence historique pour l'intégration implicite de tissus raides. Pas de reprise du code. |
| [Kim, FEM Baraff–Witkin Cloth, 2020](https://www.tkim.graphics/FEMBW/) | La formulation anisotrope et les Hessiennes indéfinies motivent les tests d'objectivité et de dérivées. Notre loi de Green est distincte. |
| [Stable Discrete Bending, 2023](https://wangzhendong619.github.io/papers/Wang-2023-SDB.pdf) | Notice/abstract retrouvés, téléchargement intégral via outil web en échec. À approfondir pour la dégénérescence et la projection spectrale ; non implémenté. |
| [Data-Driven Elastic Models for Cloth, 2011](https://wanghmin.github.io/publication/wang-2011-dde/Wang-2011-DDE.pdf) | Calibration expérimentale et réponse non linéaire : les valeurs de démonstration actuelles ne doivent pas être présentées comme tissu mesuré. |
| [Projective Dynamics, 2014](https://www.projectivedynamics.org/projectivedynamics.pdf) | Référence identifiée ; PDF intégral en échec via l'outil. Un futur solveur PD devra préciser quelles énergies sont compatibles avec ses projections. |
| [VBD, 2024](https://ankachan.github.io/Projects/VertexBlockDescent/index.html), [Gaia](https://github.com/AnkaChan/Gaia) | Futur backend de descente par blocs partageant le modèle et les métriques de référence ; comparer à résidu et loi constitutive identiques. |
| [AVBD, 2025](https://graphics.cs.utah.edu/research/projects/avbd/) | Étudier l'augmentation pour les coutures/contraintes raides. Distinguer formulation augmentée, contact et modèle matériel. |
| [C-IPC, 2021](https://ipc-sim.github.io/C-IPC/) | Barrières, épaisseur et ACCD sont des composants liés. Notre pénalité sphère n'offre aucune de leurs garanties. |
| [ZOZO PPF](https://github.com/st-tech/ppf-contact-solver) | Dépôt et documentation repérés pour la voie Accurate ; aucune intégration ni comparaison numérique réalisée. |
| [GIPC](https://github.com/KemengHuang/GPU_IPC) | Architecture GPU IPC à étudier après validation des contacts CPU ; aucune exécution CUDA. |
| [OGC, 2025](https://graphics.cs.utah.edu/research/projects/ogc/) | Formulation par décalage des faces et bornes locales de déplacement. Ce n'est pas un simple remplacement de la barrière IPC. Aucun lien de code officiel identifié sur la page consultée ; ne pas supposer sa disponibilité. |
| [BS-Cloth, SIGGRAPH 2026](https://github.com/Simulation-Intelligence/BS-Cloth) | Dépôt identifié pour une discrétisation B-spline future ; ne pas mélanger ce changement de base spatiale avec une simple option de solveur. |
| [Multilevel Additive Schwarz, 2022](https://wanghmin.github.io/publication/wu-2022-gbm/) | Piste pour préconditionner les systèmes raides à grande échelle. Actuellement Jacobi seulement. |
| [Automatic Digital Garment Initialization, 2024](https://wanghmin.github.io/publication/liu-2024-adg/) | L'arrangement initial des pièces et leurs relations de couture conditionnent l'assemblage. Priorité après les deux pièces cousues. |
| [Genesis, Simulator](https://genesis-world.readthedocs.io/en/latest/api_reference/engine/simulator.html) | Référence d'organisation séparant solveurs et couplage. Aucun composant Genesis n'est intégré. |

ARCSim/remeshing reste à examiner en profondeur : aucune lecture de son code ni validation de licence n'est revendiquée ici. Le remeshing devra transférer configuration matérielle, masse et état sans changer silencieusement le patron de repos.

## Licences et réutilisation

Implémentation des énergies, du solveur, des tests et du renderer écrite dans ce dépôt. Aucun code de recherche tiers copié. Les licences ci-dessous ont été **lues à leur URL**, à la date de cette note ; les dépendances internes des dépôts n'ont pas été auditées.

| Dépôt | Licence du fichier racine lu |
|---|---|
| [Gaia/LICENSE](https://raw.githubusercontent.com/AnkaChan/Gaia/main/LICENSE) | Apache-2.0 |
| [ppf-contact-solver/LICENSE](https://raw.githubusercontent.com/st-tech/ppf-contact-solver/main/LICENSE) | Apache-2.0 |
| [GPU_IPC/LICENSE](https://raw.githubusercontent.com/KemengHuang/GPU_IPC/main/LICENSE) | MPL-2.0 |
| [BS-Cloth/LICENSE](https://raw.githubusercontent.com/Simulation-Intelligence/BS-Cloth/main/LICENSE) | Apache-2.0 |

Les liens sur `main` sont mobiles : figer un commit et réexaminer les fichiers/notices avant une éventuelle réutilisation. C-IPC, démos AVBD, ARCSim et OGC n'ont pas fait l'objet d'une autorisation de réutilisation vérifiée. Consulter un papier ne donne pas une licence générale sur son code ou ses données.

La GUI utilise [eframe 0.36.2](https://docs.rs/eframe/0.36.2/eframe/) (egui/winit/wgpu), avec pipeline 3D propre. Les versions résolues et la liste complète des dépendances figurent dans `Cargo.lock`. Pas de backend CUDA/compute, et aucun lancement graphique ou GPU pendant ce travail.

Les métadonnées `Cargo.toml` des dépendances téléchargées ont aussi été inspectées : eframe/wgpu, glam, rayon, serde/serde_json, clap et naga déclarent `MIT OR Apache-2.0`, tracing/tracing-subscriber `MIT`, bytemuck `Zlib OR Apache-2.0 OR MIT`. Ceci est un relevé des dépendances directes et de wgpu, pas un audit juridique exhaustif des dépendances transitives et ressources embarquées.

## Séquence d'extension

1. Contrôle visuel du desktop quand le GPU est libre ; consolidation du modèle CPU et des scènes de compression/flexion.
2. Deux patrons et coutures : désormais implémentés dans le CLI CPU et la bibliothèque, avec quadrature sur discrétisations différentes et tests de bilan des forces. Voir [recherche ciblée](research-patterns.md) et [format](template-format.md). Reste l'intégration à l'éditeur et les contours généraux.
3. BVH CPU, distances VF/EE, exclusions topologiques, identification des intersections initiales, puis friction persistante et CCD/barrières cohérentes. Valider avant multicouche.
4. VBD CPU sur les mêmes énergies ; graph coloring et comparaison à résidu égal. AVBD et PD seront des implémentations séparées avec hypothèses explicites.
5. Membrane puis flexion WGSL compute, validation f32 contre f64, profilage/transferts ; contacts GPU et MAS ensuite. Rien de cette phase n'a été exécuté sur GPU.
