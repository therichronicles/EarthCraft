# Experimental GPU globe renderer

All GPU globe rendering is **experimental**, on every platform. CPU rendering
remains the default; published release downloads exclude the experimental GPU
backend. Source builds with the `gpu` feature include a wgpu 30 compute renderer
alongside the CPU renderer.
The experimental renderer ray-casts the overview globe, calculates Mercator coordinates and LOD, samples
imagery, blends zoom levels, and applies sunlight and atmosphere on the GPU.
The render crate owns it; the egui presentation backend stays in the app.
The desktop uses eframe's default wgpu presentation independently of globe
computation, including in CPU-only globe builds. Upstream graphics backend
fallbacks remain enabled. This UI backend does not accelerate CPU globe calculations.
Adapter/device creation, texture uploads, execution and readback run on the frame
coordinator, never on the UI thread.

## Selection and fallback

CPU rendering is the default while reported Ubuntu GPU freezes remain under
investigation. Experimental GPU rendering requires explicit opt-in; automatic GPU activation
must not return until native cross-platform checks cover the reported failure.

`EARTHCRAFT_RENDERER=auto` opts into experimental GPU rendering for supported
views with at least 262,144 pixels; small frames retain CPU rendering because
complete GPU/CPU timings were similar at 512×384 on the baseline adapter.
`EARTHCRAFT_RENDERER=gpu` also tries GPU rendering for small supported frames.
`EARTHCRAFT_RENDERER=cpu` skips experimental globe GPU initialization.

GPU rendering currently requires altitude of at least 1,000 km, no geographic
grid, at most 4 million pixels, and a checked LOD bound of zoom 10 or lower.
Close views and exact grid rendering use the double-precision CPU
renderer. Both backends retain all providers, historical releases, effects,
overlays and screenshots. Camera operations, picking and overlay projection
remain double precision. GPU sky, narrow horizon and tile-seam strips are repaired
using the shared precise CPU calculations, retaining deterministic stars and
avoiding floating-point changes in tile-edge selection.

Adapter selection and device creation are fallible. Required limits and shader
validation are checked before use. Initialization failure selects CPU rendering
with an English diagnostic; it does not repeatedly retry every frame. Device
loss, uncaptured GPU errors and failed readback disable that device for the
session and render the affected frame on the CPU. Rust panics from GPU initialization
or rendering are caught at the optional backend boundary so they cannot terminate
the frame coordinator. This does not recover a native driver crash or a blocked
driver call. Restarting permits another
initialization attempt. An oversized tile working set or unusual tile dimensions
uses CPU rendering for that view, preserving the general sampler.

For a desktop build without experimental GPU globe rendering:

```sh
cargo build -p earthcraft --no-default-features --locked
```

CPU globe rendering still uses wgpu presentation and requires a working graphics
driver for the native UI.

## Resource ownership and presentation

The GPU tile atlas has a separate **128 MiB maximum** (512 RGBA 256×256 layers),
reduced to the adapter's supported array-layer limit. The atlas follows keys actually
sampled by the current view, with optional global zooms 0–3 taking leftover space.
When a view needs ready tiles not yet resident, it uploads them and rerenders
before publication, with at most two bounded passes; unchanged decoded objects reuse
their texture slots. Provider/release keys remain distinct. Slots hold weak CPU
tile references, so GPU residency does not pin another decoded cache. The frame
holds a short-lived ready snapshot while uploading. If the visible working set exceeds
atlas capacity, the CPU renders that view. This limit is additional to Moka's
256 MiB decoded target and the 128 MiB MBTiles disk target.

Reusable output, dependency and readback buffers grow to their validated high
water marks. At the maximum 4-million-pixel/zoom-10 view, output and readback total
about 36 MiB, the dependency buffer about 5.3 MiB, and its host reset data another
5.3 MiB, excluding driver allocation overhead and normal frame buffers.

The shader reports sampled and missing parent/desired tile keys and their nearest-pixel
priorities through a bounded zoom pyramid. CPU admission still sorts coarse
levels first, filters pending jobs, and preserves the admission/retry
limits. Precise CPU repair contributes its dependencies too. Compact dependency metadata accompanies pixel readback.

No network or SQLite operation runs in a shader or on the UI thread.

Completed RGBA is read back to the app preparation callback, texture
upload and immutable screenshot snapshot. Consequently this is genuine GPU
rasterization, but **not a zero-copy GPU presentation path**. UI texture upload,
label layout and overlays retain their ownership and timing costs.
Global startup warming remains satellite zooms 0–4, 341 tiles, subordinate to
visible requests; other providers stay on demand.

## Verification

Original CPU fixtures remain unchanged. Naga validates the shader and uniform
layout in headless tests. Device-independent tests also verify backend
selection and one-time failure fallback. Native tests are explicit so headless CI
never silently treats a missing adapter as a successful GPU test:

```sh
cargo test -p earthcraft-render gpu_overview_matches_cpu --locked -- --ignored --nocapture --test-threads=1
RAYON_NUM_THREADS=8 cargo test -p earthcraft-render warm_gpu_frame_time --locked -- --ignored --nocapture --test-threads=1
```

The first exercises every provider, both hemispheres/poles, dateline, sunlight,
atmosphere, loaded/pending/missing imagery, tile coverage and device-loss reporting.
The second uses isolated warm 256×256 fixture tiles and compares complete frames,
including upload, GPU wait/readback, sky/precision repair and tile dependencies.
It excludes egui presentation, overlays and live services. Always report image
error statistics together with timings; initialization and the first upload are
excluded from warm medians. Release validation on Windows and Linux remains separate from local macOS checks. The benchmark also loads 597 decoded tiles and rotates across
three hemispheric views to check that exceeding the atlas in the RAM cache does
not unnecessarily disable GPU rendering. See the [native CPU baseline](PERFORMANCE_BASELINE.md).

## Local measurement (2026-10-13)

Apple M1 Pro / Metal, optimized development build without debug information or
incremental output, eight Rayon workers, 341 warm overview fixture tiles,
satellite/sunlight/atmosphere, grid off, 14,000 km. Fifteen warm frames per size,
reused buffers, first frame excluded; no concurrent build during measurement:

| Raster | CPU median | GPU complete-frame median | Maximum channel difference |
| --- | ---: | ---: | ---: |
| 512×384 | 1.983 ms | 2.107 ms | 1 |
| 2000×1488 | 28.417 ms | 8.779 ms | 1 |

This is about 3.2× faster for the larger **warm overview raster**, not an
application FPS guarantee or a close-view improvement. Initial adapter creation,
uploads on a cold cache, overlays, UI presentation, other adapters and platforms
need separate measurements.

## Desktop checks

Local macOS checks used isolated settings and cache copies. Metal rendering,
continuous rotation, Settings during motion, sunlight, PNG export (2000×1488),
coordinate fly-to at 3 km, exact-grid CPU fallback and window resizing were
exercised. A forced-CPU instance displayed the globe without GPU initialization.
Native device destruction also verified publication of a CPU frame and session
fallback. Windows/Linux GPU behavior and packaging are not locally verified.

## Dependency upgrade validation (2026-10-14)

The wgpu 30.0.1 upgrade passed native Metal CPU/GPU parity, tile dependency
coverage and device-loss fallback tests on Apple M1 Pro. The isolated complete-frame
benchmark used the default optimized development profile (debug information enabled),
eight Rayon workers and fifteen warm frames per size; initialization and first
upload were excluded. No concurrent build ran during measurement.

| Raster | CPU median | GPU complete-frame median | Maximum channel difference |
| --- | ---: | ---: | ---: |
| 512×384 | 2.294 ms | 1.438 ms | 1 |
| 2000×1488 | 49.911 ms | 18.744 ms | 1 |

The 597-tile residency case also preserved pixels and dependency coverage across
three hemispheric views. These measurements use different build settings from
the earlier baseline and do not establish an upgrade speedup or application FPS.
Headless UI regressions passed; native desktop interaction, Windows/Linux GPU
behavior, packaging and live map services were not rechecked for this upgrade.
