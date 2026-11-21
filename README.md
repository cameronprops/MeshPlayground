MESH WORKSHOP: EARLY STANDALONE PROTOTYPE

A locally run Python desktop application. No Blender, internet, LLM, or third-party Python packages are required. Python 3 with Tkinter is required. The mesh algorithms and interface are written in Python.

START
Windows: Install Python 3 from python.org with Tcl/Tk selected. Extract this folder, open it in Command Prompt, and run:
    py mesh_workshop.py
macOS: In Terminal, change to this folder and run:
    python3 mesh_workshop.py
Some Linux distributions need the python3-tk package.

HOW TO USE
1. Open an STL or triangulated OBJ. The source file stays on disk unchanged.
2. Drag with Rotate enabled to orbit, mouse wheel to zoom.
3. For local repair select Paint faces and drag across faces bordering the desired hole. Select Painted area only. All faces adjoining a hole must be painted to repair it. Rotate to reach hidden faces.
4. For whole-mesh repair select Whole mesh. Set the maximum hole boundary edge count and select Clean and patch. This fills eligible closed boundary loops and removes degenerate and duplicate triangles. It does not move original vertices. At import, exact coincident vertices are welded and degenerate/duplicate faces removed so STL triangles connect; it does not alter coordinates.
5. Save STL under a new name. STL has no unit metadata: interpret exported coordinates in the same units as the source.
6. To make an inward shell, enter thickness in the original mesh units and click Create inward shell. Original surface points are retained, an offset surface is created, and open boundaries are joined. Shelling is separate from repair; it creates new geometry. Undo operation reverses the most recent repair or shell.

IMPORTANT LIMITATIONS
This is a first prototype, not a complete automatic scan-repair tool. Hole filling spans a boundary with triangles using planar projection. It does not infer the original shape. Boundaries with significant nonplanarity, self-crossing projection, or inconsistent winding are skipped and reported. The check does not detect intersections between a proposed patch and other mesh surfaces, so a patch can still be inaccurate or intersect the mesh. It does not detect self-intersections, fix non-manifold edges, reconstruct missing features, smooth or remesh polygons, correct all normal orientation, or guarantee watertightness or printability. Clean-and-patch may close intended openings when whole-mesh mode is selected; choose painted scope when appropriate. Painted selection targets visible faces and requires the whole loop's neighboring faces to be painted. Import triangulated OBJ only. Texture, color, materials and OBJ groups are not preserved. Shelling assumes consistently wound geometry, can self-intersect at thin regions, and the requested offset is not a guaranteed minimum wall thickness. Inspect repairs and shell in another mesh viewer or slicer before fabrication. Canvas rendering may be slow for large meshes; this is not yet optimized for dense scans.

'Zero deformation' here means existing vertex coordinates are unchanged in memory during repair. Removing defective faces and creating new patch triangles still changes local topology and adds a surface. Binary STL export uses 32-bit float coordinates and therefore can introduce small numerical rounding.

RUN ENGINE TESTS
    python test_mesh.py

IMPLEMENTATION DIRECTION
This standard-library Python version is an experimental reference. A future Rust geometry core and GPU viewport would target large scan meshes; no Rust engine is included yet. Geometry behavior and acceptance tests should guide that transition.
