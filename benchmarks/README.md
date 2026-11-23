# Mesh repair benchmark

Run `python3 benchmarks/run.py PATH_TO_UNZIPPED_TEST_MODELS --output benchmark.json`. Add `--blender` to benchmark Blender's bmesh hole-fill implementation if Blender is installed. Blender is used only as an optional comparison and is not a dependency of the Rust engine or its future app.

The fixtures are the separate Mesh_Repair_Test_Models.zip supplied in the conversation. The script measures boundary counts, degenerate counts, exact face and vertex preservation, and elapsed patch time. With Blender it also counts triangle overlaps excluding faces sharing vertices. Comparison against reference triangles is not a measure of surface deviation: a valid patch can use different triangulation. The source OBJ benchmark flags ten extremely small teapot triangles under its fixed area threshold. The Rust binary STL importer reports zero exact-zero-area triangles; these use different tests and should not be treated as equivalent. Timing includes different operations and cannot be treated as a direct speed comparison.

The experimental Python patcher rejects boundaries composed of a single isolated triangle. This prevents adding a duplicate cap to the deliberately intersecting triangles. Neither benchmark method is an end-to-end repair or proves printability. The Rust engine has separate unit tests; no Rust timing is reported here.
