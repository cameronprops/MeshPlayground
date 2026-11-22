# Intersection fixtures

These are compressed binary-STL copies of the three damaged meshes generated for the MeshPlayground repair benchmark. Each contains four removed face neighborhoods, six flipped faces, and two deliberately inserted independent triangles crossing an original surface. The generated teapot source has ten exactly degenerate faces and needs cleanup before the current detector can scan it; that cleanup can change topology. These files are intentionally broken.

To run the fixture test locally:

```sh
mkdir -p /tmp/mesh-fixtures
for file in benchmarks/fixtures/*.gz; do gzip -dc "$file" > "/tmp/mesh-fixtures/$(basename "${file%.gz}")"; done
MESH_FIXTURES=/tmp/mesh-fixtures cargo test --manifest-path rust-engine/Cargo.toml --test fixture_intersections -- --nocapture
```

The test checks whether both injected triangles are reported and prints any other pairs without assuming they are false positives. It does not establish complete self-intersection detection: pairs sharing a vertex are skipped, and no independent exact-predicate reference is used. Timings are informational and depend on hardware and debug build settings.
