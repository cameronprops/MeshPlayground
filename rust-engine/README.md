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
