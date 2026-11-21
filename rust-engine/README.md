# Rust geometry engine

This is the first piece of the planned standalone Rust application. It is an independent geometry library and small command-line program using only the Rust standard library. It does not require Python at runtime or connect to the internet. The Python UI remains a separate experimental reference and does not call this engine.

This first increment reads binary STL, welds exact matching STL vertices for topology, reports topology diagnostics, and removes exactly degenerate and duplicate triangles. The library supports a per-face selection mask for cleanup. Existing vertex coordinates remain unchanged in memory. It does not patch holes, shell, paint, or render yet. STL export stores float32 coordinates and may round values. STL does not encode units.

```sh
cargo test --manifest-path rust-engine/Cargo.toml
cargo run --manifest-path rust-engine/Cargo.toml -- inspect input.stl
cargo run --manifest-path rust-engine/Cargo.toml -- clean input.stl output.stl
```

The input and output must be different files. Inspect results before fabrication. Duplicate detection compares face vertex indices, not near-coincident geometry. Degeneracy means exactly zero area; near-zero tolerance handling is a future design decision.

Performance is not benchmarked. Topology currently uses a sorted edge map, which prioritizes deterministic reports; profiling will inform changes. This increment is designed to establish testable behavior, not to claim scan-scale performance.

## User-directed preservation

MeshPlayground is intended to help a person inspect, select, preview, and accept repairs. The program does not infer that an opening should be closed based on size. The library now supports a read-only cleanup proposal that lists original face IDs to be removed. Applying it returns a separate mesh, preserves all original vertex positions and unmodified faces, and rejects a proposal if its source mesh has changed since review. The existing command-line `clean` operation is an explicitly requested whole-mesh action; it is not a GUI preview.

The current proposal retains a full source snapshot for reliable stale-proposal detection. This is intentionally simple but duplicates memory. A future scan-scale implementation needs stable mesh revision IDs or another measured approach; performance has not been established. A vertex-preserving patch can still approximate the missing surface incorrectly or intersect other geometry. Patch preview, surface deviation measurement, intersection tests, and user acceptance are not implemented yet.

Coordinates have no intrinsic units. No default movement tolerance is assumed. STL export has float32 precision and can change coordinate values through rounding.

## Element selection

The library represents vertex, edge and triangle-face selections. `Mesh::selected_boundaries` discovers connected open-boundary components touching the selected elements without modifying the mesh. A selected interior edge does not imply selecting the nearest boundary. Returned components include original vertex, edge and adjacent-face IDs and distinguish closed loops from branched boundaries. Selecting a boundary is for inspection; it never patches it automatically. Selection is currently a library API, not a visual interface. Mesh IDs are revision-specific and must be discarded or remapped after topology changes.

## Pointer selection groundwork

`Mesh::pick` finds the nearest triangle intersected by a supplied 3D ray and can identify its face, nearest edge or nearest vertex. Vertex/edge hit tolerance is specified in mesh units at the surface intersection. `Selection::apply` supports replace, add, remove and toggle operations, validating the target before changing the selection. A viewport must generate the ray and convert pixel hit radius to world units; neither the viewport nor pixel-space hit testing is implemented. Picking currently scans every triangle and is not optimized or benchmarked for large scans. Selecting an edge or vertex requires the ray to intersect a triangle, so silhouette and screen-space picking need further work. Geometry is not modified.
