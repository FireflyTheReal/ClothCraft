# Third-party material

## MakeHuman assets

`assets/makehuman/base.obj` comes from the MakeHuman community repository at revision `a8bc2d54ff0ac92e78ff71431b1023eda42bf482`. The base mesh is CC0, as distinguished from program code in the [upstream notice](https://github.com/makehumancommunity/makehuman/blob/a8bc2d54ff0ac92e78ff71431b1023eda42bf482/LICENSE.md).

The asset notice is retained at [LICENSE.ASSETS.md](assets/makehuman/LICENSE.ASSETS.md), with [provenance](assets/makehuman/provenance.json). `scripts/prepare-avatar.py` extracts the body, removes helpers, triangulates and normalizes to 1.75 m; `mannequin.obj` and `measurements.json` are derived outputs. No MakeHuman program code is incorporated.

## Dependencies and references

Rust dependencies are resolved by `Cargo.lock` and retain their respective upstream licenses. They are not vendored here. Consult each dependency's manifest/notices when preparing a binary distribution.

Research and public manual references are recorded in `docs/research.md`, `docs/research-patterns.md`, `docs/ogc.md` and `docs/manual-study.md`. Citations do not imply that upstream implementations, assets or trademarks are included or licensed by this project.

## ClothCraft source

ClothCraft source is distributed under the MIT license; see [LICENSE](LICENSE).
The MakeHuman CC0 notice applies to the bundled assets, not the program source.

## Optional Box3D research harness

`experiments/box3d` contains ClothCraft's comparison harness. It requires a
separate checkout of Box3D and does not vendor that project's source. Box3D
retains its own MIT license; keep its notice when distributing a combined
build. The pinned source and experimental limitations are recorded in
[the solver laboratory](docs/solver-lab.md).
