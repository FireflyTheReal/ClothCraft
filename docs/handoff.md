# Reprise du dépôt

État au 14 octobre 2026. GPU autorisé par l'utilisateur ; tests ordinaires CPU uniquement. Lire [AGENTS.md](../AGENTS.md), [architecture](architecture.md), [OGC/VBD](ogc.md), puis [maillage](meshing.md).

## Première publication et édition

Lire [l’accueil IA](../AI_START_HERE.md), [la roadmap](../ROADMAP.md), [le guide d’édition](authoring.md) et [l’étude des manuels](manual-study.md). La première tranche permet de dessiner des polygones, choisir des coutures entre bords, déplacer/tourner au gizmo et annuler/rétablir. Les commandes CPU valident le document avant remplacement ; le worker ignore les anciennes générations. Les formats industriels et le contrat commun de backends restent à construire.

Validation de cette tranche : [63 tests CPU, parcours natif de 20 actions et cadence réelle](publication-validation.md).

## Passe Turbo

Lire [les mesures et contrats de performance](turbo-perf.md). La détection reste f64 CPU : deux racines BVH, refit mobile, cache conservatif de candidats et réduction sur deux tâches. Les forces/rayons restent recalculés à chaque pas. Le GPU utilise un workgroup de 32 threads par sommet, des allocations réutilisables et un cache de termes matériels. L’atelier possède maintenant un worker cadencé indépendamment du rendu, un unique résultat en attente et un test `--ui-perf-smoke` sur la vraie fenêtre. `scripts/turbo-perf.ps1 -UiPerf` reproduit les essais.

## Chemin actuel

- `cloth-fit` utilise maintenant VBD + OGC par défaut. Sans `--gpu-sim`, la CLI reste f64 CPU ; l'atelier utilise le backend hybride GPU sur un worker et laisse l'interface répondre.
- `--reference` conserve Newton/PCG et son ancien contact sommets/avatar. Les résultats dans `first-fit.md` et `first-fit-results.json` sont historiques, avant OGC et avant l'élargissement des patrons.
- `ogc/` implémente régions admissibles, VF réciproque, EE, auto-contact, activation quadratique/logarithmique et bornes conservatives. `vbd.rs` utilise les mêmes énergies FEM/coutures que la référence.
- `gpu_vbd.rs` + `vbd.wgsl` calculent les forces et mises à jour VBD sur GPU. La détection, la coloration et l'empaquetage restent CPU. Aucun backend AVBD, PD, IPC ou MAS.
- `atelier.rs` : bibliothèque, 3D à gauche des patrons 2D, propriétés à droite ; saisir un triangle par ses barycentriques au bouton gauche, orbite au droit, pan au milieu. Espace : play/pause. L'attachement est une énergie, pas un déplacement forcé qui ignorerait les contacts.
- `render.rs` : avatar résident en GPU, matrice caméra appliquée en WGSL, normales interpolées. Le rendu et la simulation ont des mesures distinctes.
- Les patrons T-shirt/robe ont été élargis. Leur arrangement initial est constitué de plans séparés devant/derrière le mannequin, sans le warp par sommet qui pouvait créer des intersections entre les sommets. Les anciennes scènes intersectées sont refusées par OGC ; reconstruire un arrangement.
- Templates v1/v2, repos 2D en mètres, matériaux orientés par panneau, Delaunay contrainte et exports sont conservés. Mannequin MakeHuman CC0 de 1,75 m / 26 756 triangles.

## Reproduire

```powershell
.\scripts\check-cpu.ps1
.\scripts\start-atelier.ps1
.\scripts\ogc-benchmark.ps1 -Gpu -Steps 360 -MeshMm 30
```

La validation numérique GPU est explicite : `cloth-fit --gpu-sim --check-gpu-gradients`. Elle compare énergies et gradients f32 à l'AD f64 avant chaque pas et n'est pas une mesure de performance. `--gpu-benchmark` mesure uniquement le rendu ; `--gpu-sim` exécute la physique GPU. Le CLI expose `--dt` et `--iterations`.

Lire [les contrats OGC](ogc.md) avant de modifier le solveur. Les rapports complets sont sous `output/ogc-gpu/` ; les mesures retenues sont décrites dans [validation OGC](ogc-validation.md). Un screenshot pris avec `--steps 0` sur un projet sauvegardé montre cet état sauvegardé, pas nécessairement l'arrangement neuf.

## Priorités restantes

1. Porter la détection VF/EE, le refit BVH et la construction des listes sur GPU, avec tests contre les distances f64 et gestion explicite de tout débordement. Les reconstructions de candidats restent la principale source de pointes, même après le cache conservatif. Ne pas annoncer un solveur entièrement GPU.
2. Les allocations et caches matériels sont maintenant réutilisés, et les groupes coopératifs corrigent la faible occupation. Reste à éviter les transferts complets par pas, puis à valider des scènes denses et animées. Les timestamps `gpu_compute_ms` excluent CPU et transferts.
3. Les bornes isotropes freinent le glissement ; le backend hybride les renouvelle une fois par pas. Étudier renouvellement GPU et méthodes directionnelles séparément, sans prétendre qu'elles sont déjà présentes.
4. Les coutures restent des pénalités finies. Les exclusions sont locales aux correspondances cousues ; pas de garantie pour ces paires. Ajouter une topologie cousue ou un couplage augmenté pour fermer exactement sans dégénérer les triangles.
5. Friction, avatar animé, épaisseur multicouche et calibration matérielle restent absents. Les forces OGC sont par paire et leur calibration dépend de la résolution.
6. Conserver les tests physiques : différences finies, action/réaction, objectivité, raccord C2 interne, traversées VF/EE, cumul des bornes, état initial invalide, pins et saisie souris. Le diagnostic d'intersection emploie des tolérances flottantes, pas des prédicats exacts.

## Publication

Dépôt de première publication : `git@github.com:erythrofr/ClothCraft.git`, branche `main`. `output/` et `target/` sont ignorés. La licence du projet appartient au propriétaire. Aucun code des auteurs d'OGC/IPC Toolkit n'a été incorporé ; les sources sont citées dans `ogc.md`. Spade reste fixé dans Cargo ; provenance MakeHuman sous `assets/makehuman/`. La CI CPU est configurée dans `.github/workflows/cpu.yml` ; consulter son statut sur GitHub plutôt que supposer sa réussite.
