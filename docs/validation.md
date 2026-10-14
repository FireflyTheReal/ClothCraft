# Validation Phase 1 — 14 octobre 2026

**Archive des premières passes CPU.** Les restrictions GPU et fonctionnalités absentes décrites ci-dessous correspondent à ces exécutions historiques. L'état courant inclut l'avatar, la Delaunay et le rendu vérifié : voir [premier essayage](first-fit.md) et [maillage](meshing.md). La passe courante valide 40 tests CPU, Clippy sans avertissement sur CPU et desktop, puis des captures natives/GPU séparées.

## Conditions

Windows, Rust/Cargo 1.97.1, AMD Ryzen 7 2700X Eight-Core Processor. Les simulations ci-dessous utilisent le binaire `release` construit **sans la feature desktop**, quatre threads Rayon, `dt = 1/60 s`, un patron de 1 × 1 m avec 10 × 10 cellules (121 sommets, 200 triangles), les paramètres par défaut et 120 pas (2 secondes simulées).

Aucun test GPU, adaptateur wgpu, fenêtre graphique ou programme CUDA n'a été lancé. Le GPU de l'utilisateur était réservé à un autre travail. Ces durées sont celles d'une exécution locale sur une machine non isolée ; elles ne constituent pas une étude statistique ni une comparaison CPU/GPU.

## Commandes effectivement exécutées

```powershell
cargo build --locked -j 4
cargo test --locked --no-default-features -j 4
cargo clippy --locked --all-targets -j 4 -- -D warnings
cargo fmt --all -- --check
.\scripts\bench-cpu.ps1 -Steps 120 -Resolution 10 -Threads 4
.\target\release\marvelous-cloth.exe --headless --project output\cpu-suite\sphere\project.json --steps 1 --output output\reload-check
cargo tree --locked --no-default-features -e normal --prefix none
```

La suite compile aussi `cargo build --locked --release --no-default-features -j 4`. L'arbre normal du build sans desktop ne contient ni eframe, egui, winit ni wgpu. Le test de shader utilise uniquement le parseur/validateur Naga sur CPU.

Le build desktop a été adapté aux API courantes de `glam` (module caméra), egui (panneaux) et wgpu (pipeline/profondeur). Un signalement Clippy sur la règle du produit de la différentiation automatique est explicitement autorisé à cet endroit, avec justification ; le contrôle final réussit avec `-D warnings`.

## Tests réussis : 17

Seize tests numériques/intégration dans `tests/numerics.rs` :

- gradients/Hessiennes de membrane en tension, compression et repos, flexion et contact, comparés aux différences finies ; symétrie des Hessiennes ;
- repos plat sans force, masse intégrée égale à densité × aire ;
- énergie invariante sous mouvement rigide, transformation des forces, bilan force/couple interne ;
- permutation chaîne/trame par rotation du droit-fil de 90° et réponse plus souple en compression ;
- chute libre conforme à Euler implicite avec damping ;
- points fixés exacts, réduction de l'objectif et convergence sur 20 pas suspendus ;
- drapé sur sphère avec contact effectif pendant 35 pas ;
- pénétration initiale peu profonde signalée et réduite ;
- état conservé après un pas invalide, rejet des triangles dégénérés ;
- budget Newton atteint distinct d'une convergence ;
- sérialisation JSON, export OBJ/UV et rejet de paramètres invalides ;
- flexion presque plate résolue jusqu'à une perturbation de position de `1e-10 m`, énergie positive sans clamp.

Un test dans `tests/shader.rs` valide syntaxe et sémantique du shader WGSL sans périphérique graphique. Le chargement effectif du JSON produit par le CLI a également été exécuté avec un pas headless convergé.

## Mesures finales

Valeurs copiées des résultats réels dans [phase1-results.json](phase1-results.json). Le script peut écraser les fichiers `output/cpu-suite/` lors d'une nouvelle exécution ; cette copie conserve les valeurs de cette session.

| Scène | Pas convergés | Moyenne CPU / pas | Résidu RMS final | Déformation axiale max finale |
|---|---:|---:|---:|---:|
| Sphère, droit-fil 0° | 120/120 | 31,330 ms | 3,418e-10 N | 2,160 % |
| Suspendu, 0° | 120/120 | 12,313 ms | 9,043e-12 N | 7,234 % |
| Suspendu, 45° | 120/120 | 16,721 ms | 2,560e-9 N | 5,909 % |
| Suspendu, 90° | 120/120 | 9,935 ms | 6,283e-7 N | 4,414 % |

Tolérance au dernier pas : `1e-6 N`. La déformation affichée est `max(|lambda_warp-1|, |lambda_weft-1|)` sur les triangles, pas une borne sur toutes les valeurs singulières de F. Les temps différents entre angles reflètent aussi leurs nombres d'itérations et vitesses de relaxation ; ce ne sont pas des benchmarks entre solveurs.

Pour la sphère : pénétration maximale des sommets sur toute la trajectoire **0,750330 mm**, pénétration finale **0,178627 mm**, 53 sommets en contact à la fin. Énergies finales : membrane 0,008915918 J, flexion 0,005072804 J, contact 0,000114867 J. Le stockage estimé des tableaux d'éléments au dernier pas est de 561 664 octets ; ce n'est pas la mémoire totale du processus.

L'énergie de flexion a été reformulée algébriquement avec la différence des normales unitaires pour éviter les valeurs négatives de l'ordre de `1e-18 J` dues à l'annulation de `1-cos(theta)` au repos. Les tests et la suite de quatre scènes ont été réexécutés après cette correction ; le tableau ci-dessus correspond à la version corrigée.

Les sorties comprennent les OBJ avec UV, projets JSON, résumés et séries complètes de métriques par pas. Le binaire desktop est construit dans `target/debug/marvelous-cloth.exe` ; le binaire CPU seul dans `target/release/marvelous-cloth.exe`.

## Ce qui reste à valider

La fenêtre, l'interaction réelle et le rendu sur le pilote GPU n'ont pas été testés visuellement. La compilation et Naga ne remplacent pas ce contrôle. La vérification visuelle est reportée jusqu'à disponibilité du GPU.

Les tests ne prouvent ni absence de traversée, ni robustesse sous toutes les grandes déformations. Le contact actuel ne mesure que les sommets contre la sphère ; les faces/arêtes, self-collisions, CCD, friction, couches et avatars sont absents. La flexion directionnelle est simplifiée et les paramètres ne sont pas calibrés. Aucun résultat 10k/50k/200k sommets, GPU compute, VBD/AVBD/PD ou IPC n'est revendiqué.

## Incrément templates et coutures — 14 octobre 2026

Le pipeline multi-panneaux a été implémenté et vérifié sur CPU uniquement. Les 31 tests comprennent les 17 tests précédents, 11 tests de templates/coutures et 3 tests d'attachements matériels. `scripts/check-cpu.ps1` a réussi : formatage, tests, Clippy sans avertissement et validation des deux exemples.

Après intégration, `cargo build --locked -j 2`, `cargo clippy --locked --all-targets -j 2 -- -D warnings` (features desktop par défaut) et `cargo fmt --all -- --check` ont également réussi. Ces commandes compilent et analysent le renderer sans l'exécuter. La CI GitHub est préparée, pas exécutée à distance.

Les nouveaux tests portent sur l'intégration analytique d'une ouverture constante/linéaire avec maillages différents, gradients/Hessiennes par différences finies, symétrie et forme quadratique positive, bilans de forces/couples, invariance rigide, inversion de bord, intervalles partiels, crans, easing explicite, rejets des entrées invalides, sommets communs, masses/droit-fil indépendants, sérialisation, résolution multi-matière comparée à des résolutions indépendantes et échec transactionnel pour une couture compilée invalide. Les guides sont vérifiés sur une surface pliée à 90° et après changement de résolution.

Commande réellement exécutée, binaire release sans feature desktop :

```powershell
.\target\release\cloth-template.exe simulate examples/templates/two-panel.json --steps 30 --threads 2 --output output/template-simulation
```

| Mesure | Résultat |
|---|---:|
| Panneaux / sommets / triangles | 2 / 60 / 80 |
| Résolutions des panneaux | 3×4 et 4×7 cellules |
| Couture / échantillons Gauss | 1 / 20 |
| Pas convergés | 30 / 30 |
| Temps physique | 0,5 s |
| Ouverture maximale initiale | 96,4197 mm |
| Ouverture maximale finale | 1,7740 mm |
| Ouverture RMS finale | 0,6255 mm |
| Résidu RMS final | 9,8055e-9 N |
| Tolérance finale | 2,8534e-6 N |
| Énergie de couture finale | 5,8684e-4 J |
| Moyenne CPU par pas | 29,75 ms |

Valeurs complètes conservées dans [template-results.json](template-results.json). Le temps est indicatif sur la machine de travail avec deux threads, pas un benchmark de capacité. Les deux panneaux sont placés explicitement en 3D ; le panneau gauche a deux coins fixés. `k=5000 N/m²` est une valeur de démonstration non calibrée. La tolérance inclut le terme relatif au résidu initial du pas.

`local-ease.json` a été validé et exporté avec avertissement de longueurs locales différentes ; aucune revendication de fronces physiques ou de simulation validée n'est faite pour cet exemple. Le SVG généré a été analysé comme XML et les 80 repères de fibres de la simulation ont été vérifiés valides. Aucune fenêtre ni aucun adaptateur GPU n'a été créé.

La convergence Newton ne prouve ni l'exactitude industrielle d'une couture, ni l'absence d'auto-intersection : le raccord est une pénalité, les panneaux n'ont pas de contact mutuel et les surépaisseurs/plis/points ne sont pas modélisés. La GUI reste à un panneau ; le nouveau format est disponible dans la bibliothèque et le CLI.
