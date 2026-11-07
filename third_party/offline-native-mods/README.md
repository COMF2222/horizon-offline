# Optional native mods for Horizon Offline

Mouse Tricks and TreeFall are original MIT-licensed mods by d3grando:

| Mod | Upstream version | Source |
| --- | --- | --- |
| Mouse Tricks | 0.2.0 | [99fa1d4](https://github.com/d3grando/mousetricks/tree/99fa1d49216877506b0e5124d42a785d5ace1e09) |
| TreeFall | 0.3.0 | [43fe0a4](https://github.com/d3grando/treefall/tree/43fe0a419c55e9c76507cc03ea30ce107531f1c2) |

Each folder retains its upstream license and documentation. Copyright and SPDX headers
identify the copied Rust and TOML files. Compatibility changes for Horizon Offline are
kept here with the sources, so the Windows build does not depend on moving mod branches.

These sources are compiled into the optional game profile. They are not Java mods.
The release stager binds their manifests to the exact Horizon Offline source commit.
TreeFall's adapter uses the current mutable mining-speed API and the loaded world's
actual height range, including its exclusive upper bound.
