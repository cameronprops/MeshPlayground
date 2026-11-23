# Geometry exchange: common model and format mapping

Status: design research, not a claim that these imports or exports exist. The current Rust kernel reads binary STL triangle geometry. The goal is to share concepts and operations where they really match, preserve richer representations where they do not, and report conversions or unsupported data rather than silently dropping them.

## Common internal concepts

1. **Document and units:** optional source format/version, declared or unknown length unit, coordinate system, precision and provenance. Unit conversion must be explicit and checked; an absent unit is not implicitly millimeters. SVG user units and viewport/layout context need special treatment.
2. **Object/scene node and instance:** a named entity may reference reusable geometry with a transform; keep definition separate from placed instances when present. Not every format supports hierarchy or instancing.
3. **Geometry representations:** indexed polygon/triangle meshes; curves and paths; exact/analytic surfaces and topology (B-rep); and 2D drawing elements. An object may have more than one representation, for example exact geometry and a derived display mesh. Do not replace the exact source with the display mesh.
4. **Attributes with domains:** labels, groups/layers, materials, colors, normals, and UVs can belong to a whole object, face, vertex, or face-corner. Preserve the domain and independent indices when the source requires it. Names from different formats are not automatically equivalent.
5. **Provenance and conversion report:** track source object/element IDs when possible, mapping of edits and triangulation, user-selected tolerances, unsupported entities, assumptions, dropped attributes, and approximations. A target format cannot carry data it does not support.

## Format crosswalk

| Format | Primary overlap | Important distinctions and conversions |
| --- | --- | --- |
| STL | Surface triangles; face winding | Typically geometry only, no standardized physical units or scene graph. STL is a triangle soup; welding duplicate coordinates changes topology and must be policy-controlled. Binary STL uses float32 coordinates. |
| OBJ | Mesh vertices, polygon faces, optional UVs, normals, object/group and material references | Can contain polygons beyond triangles and free-form geometry. Face-corner indices and external MTL/textures need preservation. OBJ-to-STL requires triangulation and drops unsupported attributes. Unit conventions are not standardized by common OBJ usage. |
| 3MF | Triangle meshes; objects/components, transforms, units, properties | Core mesh uses indexed vertices/triangles and model units; components can instance objects. Properties and extension data require additional adapters; required unsupported extensions must not be silently ignored. |
| FBX | Scene hierarchy, transforms, mesh geometry, material and texture mappings | Can include cameras, animation, skinning and other data in addition to geometry. Versions/encodings and SDK or library licenses need review. A mesh-only conversion may discard rigging or animation and must report it. |
| STEP | CAD geometry, topology, units and, depending on application protocol and reader, assemblies and metadata | B-rep/analytic surfaces are not triangle meshes. STEP-to-STL requires tessellation with explicit tolerance and loses exact CAD representation. Mesh-to-STEP faceted output differs from fitting editable surfaces. |
| DXF | Drawing entities, layers, curves and potentially 3D faces/meshes | Supports many entities, including 2D/3D geometry, text and dimensions. Some 3D solid entities include proprietary modeler data. Object/world coordinate systems and entity semantics matter; support must be enumerated per entity and version. |
| SVG | 2D paths/curves, groups, transforms and styles | A 2D document can be imported as curves; making a 3D solid needs an explicit operation such as extrusion, plus a Z dimension. Viewport and user-unit resolution matter. Text/fonts, clipping and effects require explicit handling. |

## Practical conversion contracts

- Mesh to mesh: carry positions, topology where representable, per-corner attributes and object transforms. If triangulating polygons, record the source face and diagonal choices. Exporting to STL discards structure/attributes that STL cannot represent.
- CAD to mesh: keep the exact geometry in the document; generate a separate tessellation using caller-defined chord/angle controls and validate the approximation. Do not describe it as exact CAD preservation in STL.
- Mesh to CAD: offer either a faceted representation or fitted surfaces with reported deviation, complexity and unsupported features. Smoothness is never proof of accuracy; missing data cannot be independently verified.
- Curves to drawing: preserve true curves where the target supports them. If flattening to line segments, state tolerance. 2D-to-3D requires an explicit plane and, for a solid, an explicit operation/depth.
- Scene to reduced format: preserve transforms/instances as long as possible; flatten or duplicate only when requested/necessary and report material, metadata, hierarchy and precision loss.

## Primary references

- [3MF Core Specification](https://github.com/3MFConsortium/spec_core/blob/master/3MF%20Core%20Specification.md)
- [Autodesk DXF entity reference](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-7D07C886-FD1D-4A0C-A7AB-B4D21F18E484.htm), [3DSOLID data](https://help.autodesk.com/cloudhelp/2023/ENU/AutoCAD-DXF/files/GUID-19AB1C40-0BE0-4F32-BCAB-04B37044A0D3.htm), and [coordinate systems](https://help.autodesk.com/cloudhelp/2024/ENU/AutoCAD-DXF/files/GUID-D99F1509-E4E4-47A3-8691-92EA07DC88F5.htm)
- [Autodesk FBX scene elements](https://help.autodesk.com/cloudhelp/2018/ENU/FBX-Developer-Help/welcome_to_the_fbx_sdk/supported_scene_elements.html) and [scene hierarchy](https://help.autodesk.com/cloudhelp/2018/ENU/FBX-Developer-Help/nodes_and_scene_graph/fbx_scenes.html)
- [W3C SVG paths](https://www.w3.org/TR/SVG2/paths.html) and [coordinate systems](https://www.w3.org/TR/SVG2/coords.html)
- [Open CASCADE STEP processor](https://sso.opencascade.com/doc/occt-6.8.0/overview/html/occt_user_guides__step.html)

Implementation policy: detect versions and extensions, build small fixtures that demonstrate each supported behavior, and publish a capability matrix distinguishing tested support, approximation, unsupported data and preservation-only behavior.
