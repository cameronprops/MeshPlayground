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

### Expand a clicked edge to its boundary

After selecting an edge, `Mesh::expand_selected_boundary_edges` adds the rest of each connected closed boundary to the edge selection. The operation is explicit: a click by itself remains a single-edge selection. The function preserves other selections and mesh geometry, reports branched boundaries without expanding them, and returns an error without modifying the selection for invalid edge IDs. This is a geometry-engine API; the viewport and its selection command are not built yet.

## Experimental flat patch proposal

`Mesh::propose_flat_patch` previews a fan triangulation of exactly one explicitly selected, complete, closed and consistently wound convex boundary. The caller supplies a planarity tolerance in mesh units. Concave, collinear, self-intersecting, nonplanar beyond tolerance, incomplete or branched boundaries are rejected. The returned proposal lists new triangles and maximum boundary deviation from its fitted plane; it never changes existing vertices or removes faces. Applying the preview returns a separate mesh only if the source is unchanged. This is an engine API, not a GUI. It does not detect collisions with surrounding geometry or infer the intended missing shape. Preview and inspect the result before use. The prototype checks intersections and winding with simple scans and has not been optimized or benchmarked for large scan boundaries. A nonzero tolerance permits slightly nonplanar boundaries, so the resulting triangles need not form a mathematically flat surface.

## Face orientation proposal

`Mesh::propose_consistent_orientation` generates a reviewable list of face reversals needed for shared edges to have consistent winding. Applying a proposal produces a separate mesh while preserving all vertex coordinates and rejects a changed source. It refuses degenerate faces, non-manifold edges and contradictory winding constraints. It handles each edge-connected component separately and chooses the consistent orientation with fewer face flips. Consistent winding does not establish which side is outside, especially for open meshes, disconnected parts, nested shells or cavities. Outward orientation, intersection repair, nested-shell classification and printability verification are not yet implemented.

## Inspect and separate surface components

`Mesh::propose_separation` identifies edge-connected groups of faces without changing geometry. Each group reports source face and vertex IDs, open-edge counts and nonmanifold-edge counts. A chosen group can be extracted as a separate mesh with a map back to the original IDs, preserving the original coordinates and triangle orientation. Extraction rejects a changed source. Vertices shared only at a point may occur in multiple extracted meshes; loose vertices without faces are not exported. Degenerate faces must be handled first.

Connectivity is not solid classification. Intersecting surfaces that do not share indexed edges may be listed separately, nested shells may form one solid with cavities, and the engine does not yet cut intersections or compute a boolean union. The user will be able to choose to join geometry or separate parts; neither operation is inferred from this component report.

## Headless conservative repair

`meshplayground repair-safe INPUT.stl OUTPUT.stl` explicitly removes duplicate and exactly degenerate triangles, proposes consistent face winding and writes a new binary STL. It refuses to overwrite an existing output and refuses orientation if edges are non-manifold or constraints conflict. It reports before/after topology and makes no automatic assumptions about holes. It does not perform intersection detection, reconstruction, outward orientation, shelling or printability validation. Existing vertices are retained in memory, but STL uses 32-bit coordinates. Input/output are binary STL only. This is a first headless step, not a complete repair pipeline.

## Local boundary subdivision

`Mesh::propose_boundary_edge_subdivision` previews splitting one selected boundary edge at its midpoint, replacing its adjacent triangle with two triangles. It returns a separate result and preserves all original vertex coordinates. It does not smooth, alter adjacent faces, choose hole boundaries or create a hole patch. The intended surface remains the original triangle apart from floating-point rounding of the inserted midpoint; this is not a formal exact-arithmetic guarantee. A changed source invalidates the proposal. This first operation splits only one edge at a time, does not yet expose controls in a viewport, and has not been optimized for dense meshes.

Local/grid remeshing, blending a reconstructed patch into its surroundings, controlled geometric deviation checks, and sharp-feature preservation remain design goals rather than implemented features. Remeshing can alter the surface even with original points fixed; it must be previewed and measured against the original.
