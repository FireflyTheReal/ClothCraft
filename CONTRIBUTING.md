# Contribuer

Humains et agents IA sont bienvenus. Lire [l’accueil IA](AI_START_HERE.md), [la roadmap](ROADMAP.md) et [le défi solveur](docs/solver-challenge.md).

Commencer par [AGENTS.md](AGENTS.md) et [docs/handoff.md](docs/handoff.md). Les contributions humaines et IA suivent les mêmes contrats physiques et la même vérification.

Une modification physique doit expliquer l'énergie ou le contact concerné, les unités et les limites. Ajouter un test qui pourrait réfuter l'implémentation : dérivées, invariance, bilan, conservation du repos, résultat analytique ou comparaison de convergence. Pour une modification documentaire ou purement visuelle réversible, ne pas ajouter de tests qui répètent le code.

```powershell
.\scripts\check-cpu.ps1
```

La CI CPU effectue ces contrôles et un petit assemblage headless. Elle est définie dans `.github/workflows/cpu.yml` ; consulter son résultat dans GitHub Actions. Compiler l'interface avec `cargo build --locked -j 2` pour une modification de ses types, sans la lancer quand le GPU est réservé.

Conserver `Cargo.lock`. Les modèles d'exemple doivent rester petits, déterministes et annotés comme non calibrés. Ne pas versionner `target/`, `output/`, données lourdes ou caches de simulation. Les résultats de référence légers vont dans `docs/` avec leur commande et leurs limites.

Le choix de licence de distribution du projet reste à son propriétaire ; aucune licence n'a été imposée dans cette initialisation. Référencer les sources étudiées et distinguer lecture, implémentation indépendante et réutilisation de code.
