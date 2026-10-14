# Architecture et invariants

Le dépôt reste un package Rust et trois exécutables. Les modules numériques CPU n'importent ni eframe ni wgpu. La feature `desktop` ajoute le renderer, le backend compute `gpu_vbd` et les interfaces ; elle peut être désactivée pour compiler uniquement le noyau CPU.

## Chemin des données

```text
GarmentTemplate JSON
  fabric properties + panels (rest geometry / grain / placement) + seam graph
       | compile(): validation, triangulation, material binding, seam quadrature
       v
CompiledGarment = Mesh + materials + vertex material IDs + seams + panel ranges
       | initial_state.clone()
       v
ReferenceSolver::step_garment() -> State + Metrics
       |                          |
       +-- OBJ / fiber frames ----+-- material-space attachments

Project JSON v1 -> Pattern::build() -> ReferenceSolver::step()
                                      |
                          same private Newton/PCG implementation
```

L'éditeur historique utilise `Project`, un panneau. `cloth-fit` utilise `FittingProject` : template multi-panneaux, avatar, réglages et cache de positions signé. La CLI et l'interface consomment le même `FittingScene`, sans dupliquer le solveur.

Le chemin courant de `cloth-fit` passe par `VbdSolver` + `OgcWorld`, ou `GpuVbd::step` avec la même topologie OGC et les mêmes énergies. Le diagramme Newton ci-dessus reste celui du backend `--reference`. Le worker d’atelier reçoit un numéro de génération : un résultat calculé sur un ancien maillage ne remplace jamais la scène courante. Il possède l’état physique et son horloge à pas fixe, indépendamment du rafraîchissement egui. Un seul snapshot de sortie est conservé ; les mises à jour de contrôle ne réinjectent jamais l’ancien état affiché. Les ressources GPU et termes matériels sont réutilisés, et le cache des candidats OGC ne remplace jamais le recalcul des distances et bornes. Voir [Turbo](turbo-perf.md).

## Responsabilités

| Module | Rôle | Entrées / sorties |
|---|---|---|
| `authoring.rs` | Commandes transactionnelles, historique et placements rigides | template + commande → template validé + modèle compilé |
| `atelier/authoring_ui.rs`, `atelier/gizmo.rs` | Dessin, choix des coutures, sélection et gizmos | interactions → commandes ; aperçu sans remeshing par frame |
| `model.rs` | Géométrie de repos rectangulaire et maillage FEM | `Pattern` → `Mesh`, masse, charnières |
| `garment/template.rs` | Modèle d'édition, validation, compilation | `GarmentTemplate` → `CompiledGarment` |
| `garment/polygon.rs`, `garment/meshing.rs` | Contours validés, Delaunay contrainte et métrique | points/bords → UV et triangles, contrôle qualité |
| `garment/seam.rs` | Correspondance de bords et énergie de raccord | longueurs au repos → quadrature, éléments |
| `garment/embedding.rs` | Attachements géométriques en coordonnées du patron | `MaterialPath` → barycentriques temporaires |
| `garment/export.rs` | Artefacts CPU sans état d'interface | OBJ par panneau, SVG, directions de fibres |
| `math.rs`, `energy.rs` | AD d'ordre 2, membrane et flexion | géométrie actuelle + repos → énergie/dérivées |
| `contact.rs` | Pénalité sommets/sphère | contact discret, épaisseur par sommet |
| `avatar.rs` | OBJ, BVH statique et distance locale signée | mannequin → implémentation du trait Collider |
| `fitting.rs` | Arrangement, cache et pas transactionnels | FittingProject → FittingScene |
| `solver.rs` | Euler implicite, Newton/PCG, Armijo | modèle compilé + état → état/métriques |
| `project.rs` | Ancien format de scène à un panneau | `Project` JSON v1 |
| `bin/cloth-template.rs` | CLI sans fenêtre | example / validate / bake / simulate |
| `editor.rs`, `render.rs` | Interface et affichage | restent consommateurs du modèle CPU |
| `ogc/` | Régions admissibles, VF/EE, barrières et bornes | topologie + positions → contacts + rayons conservatifs |
| `vbd.rs` | Euler implicite par blocs de sommets, coloration avec contacts | mêmes énergies FEM/coutures, référence f64 |
| `gpu_vbd.rs`, `vbd.wgsl` | Solve local f32, gradients exacts, Hessiennes positives approchées | candidats/bornes CPU → compute GPU → état validé |
| `picking.rs` | Rayons et attachement barycentrique | souris → force de saisie, sans téléportation |
| `bin/cloth-fit.rs`, `atelier.rs`, `gpu_bench.rs` | CLI, atelier et mesure du rendu | projets / rapports / captures opt-in |

## Contrats à préserver

**Repos.** `Mesh.uv` est local au panneau et exprimé en mètres, sans atlas ni normalisation [0,1]. `Triangle.rest`, `inv_dm` et `area` dérivent uniquement du patron. Les placements rigides n'affectent ni ces données ni les masses.

**Matières.** Le template sépare `FabricProperties` et `Panel.grain_degrees`. La compilation crée un `Material` orienté par panneau. Triangles et charnières référencent ce matériau ; `vertex_material` fournit l'épaisseur aux contacts. La densité est déjà intégrée dans la masse : reconstruire le modèle quand elle change, ne pas modifier seulement le tableau de matériaux compilés.

**Topologie.** Les panneaux conservent des sommets distincts aux coutures. Les plages de sommets/triangles identifient chaque panneau. Le compilateur refuse les coutures qui possèdent des intervalles chevauchants du même bord ; un coin commun reste possible. Le support de coutures multicouches exigera un modèle de relation plus riche.

**Référence Newton.** Ses deux API entrent dans le même `Problem` privé. Matériaux, inertie, gravité, damping, contacts et coutures sont assemblés ensemble. Les contraintes fixées éliminent les DOF correspondants. Les erreurs conservent l'état précédent ; atteindre le budget d'itérations produit un état avec `converged=false`. Les métriques de couture sont ajoutées, pas substituées aux résidus.

**VBD/OGC.** Les mêmes énergies sont minimisées par blocs de sommets. La coloration inclut toutes les dépendances des éléments, contacts et saisies. Les centres des bornes ne changent qu'à une nouvelle détection ; les propositions locales sont tronquées puis contrôlées énergétiquement. Le backend GPU f32 vérifie les résultats et les bornes avant de modifier `State`. Le budget fixe ne signifie pas convergence. Les réglages VBD de la CLI sont enregistrés dans `run.json` ; `FittingProject.solver` conserve les réglages du Newton historique.

**Attachements.** `MaterialPath` garde un ID de panneau et des points 2D. `embed_path` scinde les segments aux traversées de triangles avant interpolation barycentrique : un guide suit donc un pli au lieu de traverser l'espace entre ses extrémités. Les `EmbeddedPoint` sont des caches à reconstruire après remeshing ; ils ne détectent pas automatiquement un autre maillage de même taille. La recherche actuelle parcourt les triangles, adaptée aux petits patrons ; un index spatial pourra la remplacer.

**Unités.** Longueurs m, masse kg, temps s, force N, énergie J, densité kg/m², membrane N/m, flexion N·m, coefficient de couture N/m². Les guides chaîne/trame ne sont pas un modèle de fils explicites.

## Évolutions localisées

1. Étendre les contours polygonaux aux courbes/trous, puis ajouter une métrique de résolution spatiale.
2. Transférer l'état lors d'un remeshing dynamique ; aujourd'hui le remeshing explicite reconstruit l'arrangement initial.
3. Étendre l’édition actuelle (création de polygones, coutures par segments, gizmos, historique) aux points existants, courbes, trous et coutures partielles.
4. Porter la détection OGC VF/EE sur GPU, ajouter friction et traitement dédié des coutures. Une couture n'autorise pas à exclure toutes les collisions entre panneaux cousus. Les bornes OGC préservent un état séparé ; elles ne sont pas une réparation d'intersections initiales.
5. Faire évoluer les lois constitutives à partir de mesures ou d'homogénéisation. Conserver les coordonnées matérielles et tests d'objectivité. Ne pas inventer une calibration.

L’objectif d’évolution des backends et leur protocole de comparaison sont décrits dans [solver-challenge.md](solver-challenge.md). Aucun trait commun ni ABI de plugin de solveur n’est encore implémenté.
