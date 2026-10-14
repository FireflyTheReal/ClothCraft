# Instructions de travail — MARVELOUS CLOTH

## Point d'entrée

Lire `AI_START_HERE.md`, `ROADMAP.md` et `README.md`, puis `docs/handoff.md`, `docs/architecture.md` et la documentation du module à modifier. `docs/research.md` et `docs/research-patterns.md` distinguent articles consultés, choix indépendants et méthodes non implémentées. Ne pas présenter les travaux cités comme des backends disponibles.

Le backend courant d'essayage est VBD + OGC : lire `docs/ogc.md`. La CLI est CPU par défaut, `--gpu-sim` active le calcul hybride et `--reference` sélectionne le Newton historique. L'atelier emploie `atelier.rs`. Ne pas confondre timestamp compute, temps de pas complet et temps du rendu. Les bornes OGC s'appliquent à partir d'un état initial séparé, avec exclusions de couture locales explicitement documentées.

La passe Turbo est documentée dans `docs/turbo-perf.md`. Préserver la couverture conservative du cache de paires, le recalcul des distances/bornes à chaque détection, les barrières du workgroup coopératif et l'horloge du worker indépendante du rendu. `scripts/turbo-perf.ps1 -UiPerf` reproduit le calcul et la vraie cadence de fenêtre ; il utilise explicitement le GPU.

## Exécution

Les essais locaux documentés ont été autorisés par le propriétaire. Pour une nouvelle session, coordonner l’usage GPU avec la demande courante ; une autorisation passée n’autorise pas toutes les futures charges. Garder la suite de tests par défaut sur CPU et les tests GPU opt-in. Limiter les petites validations à deux jobs/threads CPU.

Commandes usuelles :

```powershell
.\scripts\check-cpu.ps1
cargo build --locked -j 2
```

La seconde commande compile uniquement. La passe GPU autorisée se reproduit avec `scripts/first-fit.ps1 -Gpu -UiSmoke` ; les commandes CPU restent indépendantes. Vérifier le résultat de chaque commande. Aucun test GPU ne doit être ajouté à la suite exécutée par défaut.

## Invariants

- Les coordonnées 2D en mètres sont la configuration matérielle de repos. Ne jamais recalculer le repos depuis la forme 3D drapée.
- Un matériau de tissu est partagé ; le droit-fil appartient à chaque panneau. Toute modification du repos, de la densité ou du grain impose une recompilation cohérente des caches.
- Les fichiers d'édition référencent des identifiants de panneaux/bords et des coordonnées matérielles ; aucun indice de sommet/triangle persistant.
- Préserver l'API `ReferenceSolver::step`, `Project` v1 et les templates rectangulaires v1. Les polygones utilisent `GarmentTemplate` v2, l'essayage `FittingProject` v1. Voir `docs/first-fit.md` et `docs/meshing.md` pour les caches et changements de format.
- La Delaunay est calculée dans la métrique de maillage, indépendante de la matière. Préserver les contraintes de contour, vérifier les bornes de qualité et invalider tout cache d'état après remeshing.
- Les coutures participent au même objectif implicite, au gradient, à la Hessienne et aux métriques. Ne pas ajouter de projection arbitraire des positions après Newton.
- Les paramètres sont dimensionnés. Documenter résidu, convergence, pénalité et limitations ; une pénalité n'est pas une garantie d'absence d'intersection.
- Les guides de fibres sont des attachements géométriques à la surface. Ils n'ajoutent ni fils physiques, ni contacts entre fils, ni calibration mesurée.
- Garder `model`, `energy`, `garment`, `contact` et `solver` indépendants d'egui/wgpu. L'édition et le rendu consomment le modèle compilé.

## Contributions

Préserver les fichiers et modifications de l'utilisateur. Ne pas publier, pousser ou choisir une licence de distribution sans une demande explicite du propriétaire. Tester les changements physiques par des propriétés vérifiables : différences finies, objectivité, bilans, convergence et comportement aux limites. Mettre à jour le guide de reprise et les limites quand une capacité change. Éviter les modules vides, faux menus de solveurs et abstractions anticipant une implémentation inexistante.

## Édition et recherche produit

`src/authoring.rs` contient les commandes CPU transactionnelles et l’historique. `src/atelier/authoring_ui.rs` et `gizmo.rs` sont les adaptateurs graphiques. Lire `docs/authoring.md` avant de les modifier. Une prévisualisation de placement n’est pas un nouvel état de repos ; sa validation réinitialise les caches. Les manuels publics réellement lus figurent dans `docs/manual-study.md`. Ne pas annoncer les formats de `ROADMAP.md` comme disponibles. Le contrat de comparaison des nouveaux solveurs est dans `docs/solver-challenge.md`.

Le tableau unifié est dans `atelier/board.rs`, la sélection/couture 3D dans `atelier/edges_3d.rs`, les Bézier dans `garment/curves.rs`. Préserver le partage des identifiants de bords entre vues. `layout_2d` est exclu de la physique ; les poignées restent dans le repos local. Ne pas désactiver `serde_json/float_roundtrip` sans vérifier la stabilité des maillages au rechargement.
