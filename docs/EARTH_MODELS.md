# Earth models

Settings → Earth model defaults to **Realistic — WGS 84 (G2296)**, with
**Perfect Sphere** available as an alternative. Preferences persist independently
of language and distance units. Unknown/malformed settings retain the existing
selection; older preference files without a model use Realistic. Explicit saved
selections remain intact. Restoring defaults selects Realistic. Moon/custom-body geometry remains independent.

WGS 84 uses semi-major axis 6,378,137 m and inverse flattening 298.257223563.
NGA identifies G2296 as its current realization, introduced on January 7, 2024
and aligned with ITRF2020. The corresponding geographic CRSs are EPSG:10606 (2D)
and EPSG:10605 (3D); these identify coordinate representations, not different
ellipsoid shapes. See [NGA](https://earth-info.nga.mil/index.php?action=gnss&dir=gnss)
and [EUROCONTROL's CRS guide, Appendix B](https://www.eurocontrol.int/sites/default/files/2024-12/eurocontrol-coordinate-reference-systems-basic-user-guide.pdf).

This option interprets latitude/longitude on that ellipsoid. It does not transform
coordinates between realizations/epochs, apply station velocities or plate motion,
or add terrain/geoid heights. Camera altitude is height along the surface normal;
aircraft telemetry retains its provider altitude without a vertical-datum conversion.
Imagery and position sources retain their own accuracy and observation epochs.
Consequently selecting this model alone does not establish survey accuracy.

## Ownership and geometry

`earthcraft-core::earth_model` owns model identity, geometry and geodesics. Surface
geometry is separate from the choice of model/reference frame. Camera bases carry
the selected model and geodetic center. Existing world units remain mean Earth
radii, so UI distance conversions and other celestial bodies need no unit changes.

| Calculation | Perfect Sphere | Realistic |
| --- | --- | --- |
| Geographic surface position | Original unit normal | Geodetic position on the WGS 84 ellipsoid |
| Camera position/picking | Original sphere path | Surface normal height and ellipsoid intersection |
| Surface distance/bearing/destination | Spherical formulas with stable near-antipodal distance | Cached GeographicLib/Karney solver |
| Ruler path | Great-circle samples, including antipodes and tiny baselines | Ellipsoidal geodesic samples with shared route coefficients |
| Degrees/arcseconds | Original mean-radius central angle | Geocentric angular separation per leg, summed across paths |
| Flight | Original interpolation | Ellipsoidal distance, geodesic horizontal route and existing altitude easing |
| Scale bar | Original altitude policy | Surface distance between the actual displayed endpoints |

The [GeoRust GeographicLib port](https://github.com/georust/geographiclib-rs)
handles direct/inverse geodesics, including nearly antipodal routes. Its coefficients
initialize once and are shared. The UI keeps the existing undefined-bearing
convention for coincident/antipodal points and starts at poles; internal flight
routes use the solver's deterministic azimuth.
Exact spherical antipodes also have multiple shortest routes; their displayed arc
uses a deterministic plane. Both models retain exact path endpoints and return
only the start point when asked for zero segments. The shared surface intersection
helper selects the nearest forward hit and rejects invalid or zero-length rays.

Render owns SIMD intersections, geographic texture/grid sampling with separate
meridional/prime-vertical curvature radii, normal-based lighting, conservative
indexed boundary culling and ruler/aircraft projection.
Static boundary indexes remain in unit-geodetic space; conservative displacement
bounds and quadric horizon planes keep ellipsoidal candidates visible without
building another dataset index. Weather uses the same model-aware picking.
Placemark/ruler handles, grid labels and boundary/place labels use surface positions
from the displayed camera basis.

Frame state/presentation keys include the model. Overlay, aircraft and scale-bar
caches also key on the full basis, so switching models rejects incompatible work.
Scale refinement is cached for the displayed view. Settings persistence and all
existing background ownership/coalescing remain unchanged.

## Rendering cost and extension points

The CPU chooses specialized sphere/ellipsoid row kernels before entering pixel
loops. Kernel selection matches surface geometry exhaustively, so adding a surface
family requires an explicit decision. Ellipsoid ray coefficients are prepared once
per frame. There is no per-pixel virtual model dispatch. The original sphere raster
arithmetic and raster fixtures remain intact. The experimental GPU shader supports the sphere
only; WGS 84 explicitly selects the CPU path, including when GPU was requested.
Eframe's UI backend is independent of this choice.
Ellipsoid sampling reuses the unit normal's sine/cosine for curvature, texture
coordinates and grid widths instead of repeating angle/trigonometric conversions.
All-sky ray batches retain atmosphere inputs while skipping unused surface positions
and normals. Geographic calculations remain in f64, with no approximate
trigonometry or reciprocal math. Isolated microbenchmarks and before/after capture
checks are documented in [the performance baseline](PERFORMANCE_BASELINE.md).

To add another spheroidal model/reference frame, register its stable identity in
`EarthModel::ALL`, supply its labels/help, geometry and cached geodesic solver in
core, and add reference cases. The dropdown and persistence consume the registry;
frontends/projectors use the selected basis/model instead of matching model names.
Realization transformations and coordinate epochs are a separate capability and
must not be implied by adding a label.

A fundamentally different surface, such as a disc, needs a new `SurfaceGeometry`
family with its own coordinates, normals, picking, culling and distance/path rules,
plus a specialized render kernel and capability tests. It must not silently enter
the ellipsoid kernel. This is an extension boundary, not an implemented disc model.

## Validation

Automated checks cover known WGS 84 equatorial/polar/antipodal lengths, direct/inverse
round trips, geodetic surface positions, scalar/SIMD intersection agreement,
short/antipodal path sampling, a published GeographicLib direct/inverse example,
forward/inside/on-surface/invalid rays, cursor-anchored zoom, conservative culling,
scale-bar endpoint distances, settings
round trips, model-dependent frame keys and GPU capability rejection. Existing
sphere raster/missing-tile fixtures are unchanged. Tests use local data.

For isolated performance measurements, see [the baseline](PERFORMANCE_BASELINE.md).
Native interaction and public imagery/aircraft services remain separate manual
checks; no GUI or surveying accuracy is established by compiler/test success.
