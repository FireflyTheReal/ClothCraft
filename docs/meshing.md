# Maillage des patrons : Delaunay contrainte

`src/garment/meshing.rs` encapsule **Spade 2.15.1**, version fixée dans Cargo. Le patron reste la configuration de repos en mètres. Le maillage est un cache reconstructible ; les coutures continuent à référencer des noms de bords et des abscisses, jamais des indices.

## Réglages

Dans `panel.polygon.meshing` :

```json
{
  "target_edge_m": 0.025,
  "anisotropy": 1.0,
  "axis_degrees": 0.0,
  "min_angle_degrees": 25.0,
  "max_vertices": 20000
}
```

Le mode isotrope utilise `anisotropy = 1`. Une valeur 2 autorise une taille deux fois plus grande **le long de l'axe** indiqué, et conserve la taille cible dans sa direction perpendiculaire. Cet axe décrit la résolution géométrique ; il ne modifie pas le droit-fil ni les propriétés physiques du tissu.

La transformation constante utilisée est `q = (dot(p,a)/(h*r), dot(p,b)/h)`, où `a` et `b` sont les axes tournés, `h` la taille cible et `r` le rapport. La triangulation et le contrôle des angles se font dans cet espace métrique ; les UV exportés reviennent en mètres. Il s'agit d'une anisotropie globale par panneau, pas encore d'un champ spatial adaptatif suivant courbure ou contraintes.

`target_edge_m` est une taille nominale d'équilatéral, **pas une borne stricte sur chaque arête**. La borne vérifiée est `aire métrique <= sqrt(3)/4`. L'angle demandé vaut 25° par défaut dans l'espace métrique. Les angles imposés par un contour aigu peuvent empêcher cette borne angulaire ; les valeurs réellement obtenues sont rapportées. En mode anisotrope, les angles euclidiens peuvent être plus petits.

## Pipeline

1. Valider le contour extérieur simple positif et les noms uniques ; valider aussi les trous horaires et les lignes intérieures disjointes s’ils sont présents.
2. Échantillonner les bords selon leur longueur métrique et les imposer comme contraintes.
3. Raffiner la Delaunay par insertion de points de Steiner avec une borne d'aire et un angle cible. Les arêtes de contour peuvent être subdivisées.
4. Vérifier l'aire réellement obtenue. Si nécessaire, ajouter les centres des faces encore trop grandes et recommencer, au plus huit passes, avec un budget global de sommets.
5. Retirer les faces extérieures, reconstruire les chaînes de bords ordonnées et compiler masses, éléments et coutures depuis les UV.

Une limite de budget produit une erreur explicite. Elle ne renvoie pas silencieusement un maillage incomplet. Les caches d'essayage contiennent une signature du template, des UV et des triangles : une modification de maillage invalide les positions sauvegardées.

Avec `holes`/`lines`, un cadre temporaire distant de deux unités métriques sépare l’enveloppe convexe des contraintes du tissu : cela évite les angles quasi nuls dus aux arrondis. Après raffinement, les faces hors matière et sommets du cadre sont retirés. Une ligne intérieure garde du tissu des deux côtés et produit une chaîne nommée utilisable par le modèle de couture. Le calcul peut raffiner inutilement les vides et épuiser le budget ; il échoue explicitement. Les entrées historiques sans trous/lignes gardent le chemin précédent. Voir [drafting-tools.md](drafting-tools.md) et `tests/drafting_tools.rs`.

Le champ historique `subdivisions` reste accepté (0..4). Sans réglage explicite, il correspond désormais à `h = 0,1 / 2^subdivisions` m. Les nouveaux exemples enregistrent `meshing` explicitement. Le format de template reste v2, encore non publié ; le format v1 rectangulaire ne change pas.

## Vérification

`tests/meshing.rs` vérifie les contraintes de contour, la propriété locale de Delaunay sur les arêtes intérieures, l'aire et les angles métriques, la reproductibilité, la conservation de masse et de longueur de couture sous raffinement, ainsi que les erreurs de budget. Les cas anisotropes incluent un axe tourné de 37°.

Commandes et captures : [premier essayage](first-fit.md). La densité réduit les erreurs géométriques de contact, mais ne remplace pas une collision continue ni les contacts entre faces.

## Sources consultées

- [Spade — triangulation contrainte et raffinement](https://docs.rs/spade/2.15.1/spade/struct.ConstrainedDelaunayTriangulation.html) : API utilisée, insertion de Steiner et limites de qualité.
- [Paramètres de raffinement](https://docs.rs/spade/2.15.1/spade/struct.RefinementParameters.html) : aire, angle, budget et exclusion des faces extérieures.
- [Dépôt et licence de Spade](https://github.com/Stoeoef/spade/blob/master/Cargo.toml) : MIT OR Apache-2.0. Ce backend est une dépendance déclarée, pas une réimplémentation revendiquée.
