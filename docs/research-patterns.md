# Patrons, coutures et passage aux fibres — revue du 14 octobre 2026

Revue ciblée de sources primaires, destinée à orienter les prochains modules. Elle ne constitue ni une reproduction des articles ni un classement exhaustif du domaine. Les implémentations présentes sont indépendantes et décrites dans [template-format.md](template-format.md).

| Source consultée | Résultat utile | Conséquence pour ce dépôt |
|---|---|---|
| [GarmentCode, SIGGRAPH Asia 2023](https://korosteleva.com/publication/garmentcode/) et [dépôt des auteurs](https://github.com/maria-korosteleva/GarmentCode) | Construction de patrons paramétriques composables, séparation des pièces et interfaces de couture. | IDs sémantiques, panneaux et relations persistantes séparés du maillage. Notre v1 rectangulaire n'est pas une implémentation de leur DSL ni un importeur de leur format. |
| [True Seams, Rodríguez & Cirio, 2022](https://gabrielcirio.gitlab.io/projects/trueseams/trueseams.html) | Les coutures réelles dépendent des recouvrements, replis et points ; construction par problèmes 2D puis relèvement 3D. Page/abstract consultés. | Un raccord d'arêtes n'en reproduit pas les propriétés. Prévoir plus tard une couche de construction de couture avec géométrie et contacts ; ne pas la masquer derrière une raideur accrue. |
| [Automatic Digital Garment Initialization, Liu et al., 2024](https://wanghmin.github.io/publication/liu-2024-adg/) | L'arrangement initial sans replis/intersections est un problème distinct de la simulation, traité par classification, heuristiques et optimisation. Page/abstract consultés. | Conserver un placement éditable par panneau. La fermeture numérique de deux pièces ne suffit pas à assembler n'importe quel vêtement autour d'un avatar. |
| [Computational Pattern Making from 3D Garment Models, 2022](https://arxiv.org/abs/2202.10272) | Construction de patrons depuis des surfaces 3D avec contraintes de fabrication et de déformation. Notice/abstract consultés. | Une future conversion 3D→2D doit gérer les distorsions et le grain ; les UV de rendu ne sont pas automatiquement un patron physique. |
| [EASE, Bartol et al., 2026](https://arxiv.org/abs/2606.29419) | Aisance locale anisotrope comme variable de conception, distincte de la déformation physique ; optimisation de patrons sous contraintes. Notice et sections 3.1–3.3 consultées, référence Computer & Graphics vol. 138. | Séparer allowance/aisance et loi élastique. Notre `allow_ease` autorise seulement des longueurs de bords différentes ; aucun modèle EASE implémenté. |
| [Homogenized Yarn-Level Cloth, Sperl, Narain & Wojtan, 2020](https://research-explorer.ista.ac.at/record/8385) | Simulations de motifs de fils pour construire une densité d'énergie macroscopique utilisable par une coque. Notice/abstract et extraits indexés consultés ; ouverture du PDF de l'éditeur de projet en échec. | Voie cohérente pour conserver une simulation FEM tout en remplaçant la loi de matériau par une loi issue de fils. Nos valeurs actuelles ne proviennent pas de cette procédure. |
| [Neural-Assisted Homogenization of Yarn-Level Cloth, Feng et al., SIGGRAPH 2024](https://wanghmin.github.io/publication/feng-2024-nah/) | Loi constitutive apprise à partir d'énergies de déformation, attention à la stabilité et à la collecte des données. Page/abstract consultés. | Une loi apprise devra exposer énergie et dérivées compatibles avec Newton et être vérifiée hors distribution. Aucun entraînement nécessaire pour préparer cette interface. |

Le [dépôt neural_yarn](https://github.com/Rullec/neural_yarn) consulté ne montrait qu'un README ; il ne fournit pas à lui seul une implémentation réutilisable vérifiée. La page [Neural Geometry Details for Real-Time Multi-Scale Cloth Simulation](https://sueraywang.github.io/project/nngeomclothdetail/) est formulée comme une proposition et un suivi de projet : elle ne sert pas ici de preuve d'un résultat scientifique validé.

Dans [la méthode EASE, §3.2–3.3](https://arxiv.org/html/2606.29419v1), les contraintes barycentriques sont figées après initialisation LSCM ; l'optimisation locale/globale ajuste le patron avec compatibilité de longueurs et régularisation des coutures. Le drapé XPBD vient ensuite et fusionne les sommets de couture. Ce pipeline de conception est distinct de notre assemblage mécanique à sommets séparés. Le [README public EASE/LOOM](https://raw.githubusercontent.com/kristijanbartol/ease/main/README.md) décrit un pipeline Python/C++, des dépendances SMPL et des limites de fabrication. Il a été lu ; le code n'a été ni exécuté ni audité.

## Décision d'architecture

Le passage d'une feuille 2D au tissu se décompose en trois niveaux :

1. **Géométrie matérielle** : contour du patron, grain, tissu et assemblage. Le repos reste en 2D ; il survit au déplacement 3D et au changement de résolution.
2. **Mécanique effective** : F transforme les directions chaîne/trame ; une loi d'énergie calcule les forces. La loi analytique actuelle est une référence à remplacer ou calibrer, pas un tissu industriel identifié.
3. **Structure fine** : des tracés définis dans le patron peuvent être attachés à la surface par barycentriques. Cela prépare les repères, les chemins de points et le détail visuel. Une vraie simulation de fils nécessite en plus une topologie textile, flexion/torsion de fils, contacts et paramètres mesurés.

Les modules ajoutés couvrent le niveau 1 pour des rectangles, les raccords mécaniques et les attachements géométriques du niveau 3. Ils n'ajoutent pas de fils physiques. L'homogénéisation est une piste pour le niveau 2, pas un backend activé.

## Expériences suivantes sans GPU

- Contours polygonaux orientés, triangulation contrainte, IDs de bords stables et tests de transfert de couture au raffinement.
- Séparer ligne de coupe, ligne de couture et marge, avec tests d'offsets et d'intersections sur petits contours.
- Référence de loi constitutive mesurée : données explicites, unités et protocole, validation gradients/Hessiennes et objectivité avant intégration.
- Comparer différentes résolutions et pénalités de couture sur des scènes avec contacts fiables avant de conclure à une fidélité de fronces ou de surépaisseurs.
- Arrangement initial et contacts CPU VF/EE avant d'étendre les exemples à une manche ou à des couches fermées.

## Réutilisation et provenance

La [licence racine GarmentCode](https://raw.githubusercontent.com/maria-korosteleva/GarmentCode/main/LICENSE) a été lue : MIT, copyright 2024 Maria Korosteleva. Aucun code de ce dépôt ou des articles n'a été copié. Les dépendances, modèles et données de ces projets n'ont pas été audités. Les références sur `main` sont mobiles ; figer un commit et vérifier les notices avant toute réutilisation.
