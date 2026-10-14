# Contributing

Start with [the architecture](ARCHITECTURE.md) ([português](ARCHITECTURE.pt-BR.md))
and [agent rules](../AGENTS.md). Keep feature work separate from mechanical moves
so reviewers can distinguish behavior changes from file organization.

## Development

Install Rust 1.95 or newer through rustup. The locked dependency graph includes
eframe/egui 0.36, which require Rust 1.95.
The workspace uses Edition 2024 and declares Rust 1.95; CI checks stable on Linux,
macOS and Windows. Stable is suitable for day-to-day development; do not use APIs
newer than the declared minimum.

On Linux, install build prerequisites before compiling:

```sh
sudo apt install build-essential pkg-config libgtk-3-dev libxkbcommon-dev libwayland-dev libx11-dev
```

From the repository root:

```sh
cargo run --release
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

The default member is the desktop app. Plain `cargo test` does not run every
library's unit tests: use `cargo test --workspace`. For focused changes:

```sh
cargo test -p earthcraft-core
cargo test -p earthcraft-data
cargo test -p earthcraft-render
cargo run -p earthcraft
```

Keep `Cargo.lock` in version control. Normal verification uses `--locked` to
catch unintended dependency resolution changes. Intentional dependency updates
should be reviewed separately, including compatibility with Rust 1.95.
The `app-build-test` workflow runs on pushes and pull requests changing `apps/**`
or `crates/**`, and can also be run manually from Actions. It formats, lints,
tests, and builds on Linux/macOS/Windows using stable Rust. Use the latest supported major version tags for GitHub
Actions, rather than commit hashes. For actions without version tags, follow the
maintained upstream branch. Keep Rust toolchain versions explicit.

Both app checks and release builds use `Swatinem/rust-cache@v2` after selecting
the Rust toolchain. It caches downloaded Cargo dependencies and compiled dependency
artifacts, while workspace crates still rebuild. App-check caches are separated
by runner/toolchain; CPU-only release caches are separated by target architecture
and from the app-check profile. The action also keys caches by compiler and Cargo
manifest/lockfile changes. Initial runs are cold; subsequent compatible runs can
reuse their dependencies. Builds and tests still run on every workflow invocation.

For preview artifacts, version tags and publication, see [releases](RELEASING.md).

## Tests and behavior preservation

Camera tests call the same methods as the app. Boundary and search tests use
local JSON/strings. KML tests use isolated temporary files. Raster regression
fixtures cover empty-cache and loaded-tile rendering; see [fixture provenance](../crates/earthcraft-render/tests/fixtures/README.md).
Tests require no public map service or saved user configuration. HTTP fixtures
bind an ephemeral loopback port; they exercise response limits, retries and disk
cache behavior. Fluent tests verify catalog syntax, matching keys/variables and
exact UI output. See [localization](LOCALIZATION.md) before editing text.

Prefer tests of observable behavior and invariants. Avoid copies of production
algorithms. For behavior-preserving changes, retain constants, coordinate
order, persistence formats and UI strings. Do not regenerate regression fixtures
unless a deliberately approved rendering change explains the difference.

The automated tests do not validate live services, GUI event delivery, native
file dialogs, or packaging. Run the following manual checks for affected areas:

- Launch from the root and verify the icon, initial view and panels.
- Orbit, zoom under the cursor, rotate with right-drag/Shift, use arrow keys,
  compass/north reset, navigation widgets, double-click and fly-to.
  Hold each pan, rotation and zoom button for at least ten seconds; verify
  movement continues until release, including with slight pointer movement.
  Toggle Settings → Hide navigation controls; verify the top-right controls
  disappear immediately, their area accepts normal map input, reopening restores
  controls, and the choice persists after restart (default: controls visible).
- With background rendering, open Settings during continuous rotation/flight;
  verify input remains responsive, new imagery keeps arriving and borders/labels
  stay aligned. Stop input and check full-resolution recovery. Rapidly switch
  providers/effects and resize to check obsolete results, then close while busy.
- At the initial distance, let the satellite overview warm, then rotate to the
  opposite hemisphere. Check that overview imagery is available, close zoom
  still loads detail, and other layers remain on demand. Compare cold and warm
  disk-cache startup separately; do not promise immediate cold-network readiness.
- In an isolated cache directory, use Settings → Clear on-disk cache. Check
  progress/completion, retained imagery in memory, preservation of settings and
  boundary caches, and that old queued downloads do not restore deleted files.
  Check the usage readout beside the button before clearing and its refreshed
  value afterward; it counts managed MBTiles/legacy file lengths, including SQLite overhead
  but excluding filesystem allocation overhead.
  Verify that fresh visible misses and a later startup can cache tiles again.
- Switch satellite/streets/topography/historical imagery; verify loading,
  release selection, fallback and attribution.
- Search by coordinates and address; select suggestions with mouse and keyboard.
  Clear the field after results arrive and during a pending lookup; the old count,
  result list and late responses must not return. Check singular/plural result
  and KML import messages in both languages.
- Toggle borders, city/metropolitan layers, labels, grid, atmosphere and sunlight.
  Enable the grid's indented Show numbers option, zoom and rotate through the
  dateline/poles; labels must follow the displayed lines and avoid each other.
  Toggle numbers while stationary, change map text size/language and resize.
  Disable the grid: numbers disappear but their preference is retained.
- Check the bottom-right heading through rotation and flight; click it to reset
  north. Verify the top compass's localized cardinal directions rotate with the
  displayed view, clicking N resets north, and the north-up, 180° flip and held
  left/right rotation buttons remain available inside it. Drag the outer ring to
  rotate. The combined control is always used when navigation controls are visible.
  Grid labels should share a left column and bottom row inside the visible globe;
  their axes swap as the view rotates. Grid numbers default off and persist.
  Alt-click (Option-click on macOS) to start a disconnected ruler path, then
  use normal clicks to extend it. Gaps must not draw connecting segments or add
  distance; a new one-point path has no bearing. Drag/delete points in either
  path and confirm they remain separate, including after deleting every point
  in the newest path and placing another. Clear ruler removes all paths.
  Undo/redo additions, new paths, a long drag, deletion and Clear; a drag must be
  one undo step. After undo, a new edit must disable Redo; a click without moving
  a handle must preserve it. Verify buttons disable when unavailable.
  Use Cmd+Z / Cmd+Shift+Z and Edit → Undo/Redo on macOS; use Ctrl+Z /
  Ctrl+Shift+Z or Ctrl+Y on Windows/Linux. Focus a search/KML text field: its
  undo/redo must affect text without altering ruler points.
  Open Ruler → Edit coordinates. Add exact decimal-degree points, including ±90°
  latitude and ±180° longitude, and start a separate path with the checkbox.
  Select an existing point, edit it and apply; reopening/selecting must preserve
  its full input precision. Undo/redo each add/edit. Reject NaN, infinity and
  out-of-range input without changing points. Delete or undo a selected point
  on the map: Apply must require reselection, never overwrite a shifted index.
  Enter submits a coordinate field; Escape/Close cancels unsubmitted input.
  Repeat in both UI languages. Measurements retain the current spherical model.
  Add two ruler points, check the last segment's initial true-north bearing;
  add/remove/edit points and verify the bearing updates. Coincident or antipodal
  points must show an undefined bearing rather than an arbitrary direction.
- For overlay caching, toggle label/border layers while stationary, switch
  language/map text size, and resize. Verify updates without camera movement;
  late boundary patches and ruler/placemark edits must appear immediately.
- Create placemarks; save/open/drop KML; export a screenshot and use the ruler.
- In Explorer, display large planet/sky images and load/save `.earthplanet` files;
  the UI should remain responsive while the file work runs. Check the busy
  indicator and disabled file controls, including after a failed operation.
  Load a planet after choosing unrelated image paths, then save it to a new
  document and reopen it: planet, sky and name must match the loaded document.
  Repeat with no sky and a legacy sidecar document. Delete the original image
  files after importing and confirm re-saving still works. Start a load, switch
  back to Earth or the Moon, and verify completion preserves that selection.
  Choose the Moon for the first time and confirm its texture arrives without
  switching back if another body is selected before decoding completes.
- Open language, distance-unit, ruler-unit and historical-imagery dropdowns with
  a short option selected. The selected/hovered rectangle must fill every row,
  including empty space to the right, even on the first opening.
- Switch English/Portuguese, change UI/map scales, then restart to verify persistence.
  Ruler help uses Option-click on macOS and Alt-click elsewhere.

Report which checks actually ran and which platforms/services remain unverified.
For packaging, use the app manifest/package, e.g. a package-selecting command
supported by your packaging tool or run it from `apps/earthcraft`. Release
artifacts still go to the root `target/release`. macOS bundles use
`assets/macos/icon.icns`, generated from the unchanged `assets/icon.png` artwork with
transparent Dock padding and standard and Retina sizes (16–1024 pixels).
The macOS runtime icon is `assets/macos/icon.png`, with the same padding as the
bundle icon: eframe applies it to the Dock at startup. Keep both assets aligned;
Windows/Linux retain `assets/icon_256.png`.
The `packaging/earthcraft.desktop` asset must still be supplied before validating
deb output.


Distance and path smoke checks: select System, kilometers/meters and miles/feet
in Settings; check ruler segments/totals, camera altitude and close-view scale
bars. Click the bottom-bar camera altitude and enter an exact value in the shown
unit; verify Enter/Apply updates altitude without changing the center/heading and
cancels an active flight. Escape/Cancel must leave altitude unchanged. Check
invalid, non-finite and out-of-range values, and repeat in both languages. Try each explicit ruler unit and verify that both segment labels and totals
change while altitude and scale keep their own settings. Check degrees/arcseconds,
smoots and football fields (100 yards without end zones). The field option is
available everywhere: verify “Football fields” in the US selector and “American
football fields” elsewhere, with the shorter name in totals/segment labels.
Restart to check that the ruler preference persists across locales. Restart and verify the override persists; switching UI language must not
change it. System locales use the regional defaults listed in ARCHITECTURE.md. Inspect folder labels and KML
import/save/screenshot statuses: home paths use `~/…` on macOS/Linux and
`%USERPROFILE%\…` on Windows, other long paths retain their tail with `.../`.
Hover for full paths/messages. Resize the window and check toolbar spacing and
that file status text does not push language/settings controls out of view.

## Resource-constrained builds

For a small disk or a quick local check, reduce generated debug information and
incremental output without changing the shared development profile:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo test --workspace --locked
```

On PowerShell, set those names with `$env:CARGO_PROFILE_DEV_DEBUG="0"` and
`$env:CARGO_PROFILE_DEV_INCREMENTAL="false"`, then run Cargo normally.
`cargo clean --profile dev` removes rebuildable development output. Never delete
user caches, screenshots, KML, or unrelated files to make a build fit.

## Review checklist

Explain the concrete behavior affected, why the owning module is correct, and
what ran. Keep imports explicit and public APIs small. Prefer a module to a new
crate when code shares the same responsibility and dependency requirements.
Update architecture docs when ownership or dependency direction changes. Update
both READMEs when changing user-facing setup or feature descriptions. Stage
explicit files and keep generated output out of commits.

## Dependency changes and memory measurements

Use separate commits for each library migration; group dependencies only when
needed for one boundary (for example Tokio/reqwest or Fluent's supporting crates).
Verify the affected invariants before committing, then run workspace checks.
Use glam's double precision for geometry and SIMD float vectors for colors;
avoid alignment assumptions when using bytemuck with pixel buffers.

For memory comparisons, use the same build profile, viewport, settings, cache
state, enabled layers and elapsed startup time. Record which memory metric was
used. Resident memory and Activity Monitor's memory column are not interchangeable.
Moka's 256 MiB target is weighted by decoded tile bytes and enforced best-effort;
brief overshoot is expected. It does not limit total application memory, and its
weighted-size estimate can lag inserts/evictions. Keep retry/deduplication tests
independent of cache maintenance, and do not add per-frame eviction flushing.
Municipal geometry is deferred rather than removed. Exercise enabling both
municipal and metro layers after startup, and verify that loading labels finish.

For a focused cache lookup comparison (optimized dev profile):

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo test -p earthcraft-data cached_hit_throughput --locked -- --ignored --nocapture
```

This ignored manual benchmark reports median cached-hit throughput over five
rounds with 256 warm tiles and one/eight threads. It calls production lookup code;
there are no timing assertions. It excludes misses, downloads, eviction pressure,
full rendering and texture upload. Do not equate its numbers with application FPS.

To compare complete warm CPU frames using isolated disk fixtures:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false RAYON_NUM_THREADS=8 cargo test -p earthcraft-render warm_cpu_frame_time --locked -- --ignored --nocapture --test-threads=1
```

Run this benchmark alone: it injects a private imagery directory into the cache
constructor, then removes that directory after its workers stop. It does not
modify the process environment. It renders 512×384 frames with grid, sunlight and atmosphere, using 341
warm 256×256-pixel fixture tiles (85.25 MiB decoded). Startup warming uses production admission and disk
workers, and rotations across hemispheres, the dateline and poles assert that
overview frames request no new tiles. Five rounds of two hundred frames include production ray
casting, sampling, allocation and tile lookup; they exclude GUI upload, overlays,
network and eviction pressure. Neither benchmark is a substitute for live-app
frame-time and memory measurements.

For a SIMD comparison, alternate baseline and current executables with the same
compiler/profile and Rayon thread count, without concurrent builds. The generated
textures exercise a larger tile working set, but exclude eviction pressure and
real imagery variation. Ray-batch regression fixtures separately cover partial
rows, altitude extremes, poles/dateline and every provider, including exact
missing-tile keys and nearest-pixel priorities.

To measure overlay projection-cache reuse and label occupancy separately:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo test -p earthcraft overlay_projection_and_label_occupancy_time --locked -- --ignored --nocapture --test-threads=1
```

The isolated synthetic benchmark compares the old linear intersection scan with
the production R-tree for 8,000 ordered rectangles in sparse/dense screen areas,
asserting identical admissions. It also compares rebuild/reuse for 10,000
32-point geographic lines and 8,000 labels using production overlay drawing.
Medians exclude the first font/cache warmup, rendering, services, tessellation
and GPU work. Overlay rebuild measurements now include waiting for asynchronous
geometry preparation; cache reuse remains immediate. Do not compare these totals
directly with the earlier synchronous rebuild numbers. R-tree insertion/traversal can cost more at the current 500-label
cap; do not present cache reuse or this benchmark as an application FPS guarantee.

For static geographic-index comparisons, run the render-owned benchmark:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo test -p earthcraft-render static_boundary_index_time --locked -- --ignored --nocapture --test-threads=1
```

It compares candidate results with rstar and linear bounds checks over moving
views, separately from fonts, shape caching and GPU work. Geographic projection,
occupancy and frame scheduling tests live in render; egui shape/font tests remain
in the desktop app. Native checks still verify their integration.

To compare loaded-name prefix search independently of network/UI work:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo test -p earthcraft-data prefix_index_time --locked -- --ignored --nocapture --test-threads=1
```

It verifies complete suggestion parity while measuring broad, narrow and missing
prefixes over 16,000 synthetic names. Report map bytes separately from the owned
name snapshot; compare construction separately from per-query cost.

## Opt-in native profiling

Normal builds compile timing capture out. To capture a native session:

```sh
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false cargo build -p earthcraft --features profiling --locked
EARTHCRAFT_PROFILE=/tmp/earthcraft-profile.csv RAYON_NUM_THREADS=8 ./target/debug/earthcraft
python3 scripts/profile_summary.py /tmp/earthcraft-profile.csv
```

The destination must not already exist; choose a new filename for each run. File
writing runs on a bounded background recorder (8,192 queued events). Producers
never wait for writes; `dropped_events` reports saturation. Failures appear in
stderr. Closing the profiling app requests a bounded final flush. Reports contain
operation timings and dimensions/counts only, without names, tile URLs or queries.
Profiling-enabled builds require `EARTHCRAFT_PROFILE` to select an output file.
Do not use their timings as overhead-free measurements.

Finish compilation before recording measured intervals. Fix the compiler/profile,
Rayon thread count, window size/DPI, layers and cache state for comparisons. Warm
rotation, coordinate flight/detail loading, municipality/metropolitan loading and
close rotation should be separate intervals. Use an isolated copy of cached data
when changing disk/cache controls. Keep live downloads distinguishable from warm
CPU work; record resident-memory samples separately from decoded-cache estimates.

Optional phase JSON uses microseconds since the Unix epoch:

```json
[{"phase":"warm_rotation","start_us":1791904857946195,"end_us":1791904900972873}]
```

```sh
python3 scripts/profile_summary.py /tmp/earthcraft-profile.csv --phases /tmp/phases.json
```

The script groups raster resolutions separately and reports median, nearest-rank
p95 and maxima. Open-ended phases are supported for inspecting a live recording.
`frame.delivery` measures submit-to-UI-poll time, not input-to-screen latency;
`ui.texture_submit` measures app-side submission, not driver upload. UI timings
exclude eframe's subsequent tessellation/GPU work. `overlay.prepare` includes
cached returns and index rebuilding when needed. Tile lock timing samples view
admission, not every per-pixel cache miss. Avoid adding overlapping timing medians
across threads. The [native baseline](PERFORMANCE_BASELINE.md) records limitations.

Feature-enabled checks must accompany normal checks when changing instrumentation:

```sh
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
```

The two text-size sliders use relative tracks with a shared 100% midpoint.
Each keeps its existing limits and five-percentage-point steps. Check pointer
and keyboard editing, typed percentages and UI-scale application on drag release.

## Frame rate and display synchronization

Native presentation synchronizes with the current display by default.
Settings → Uncap FPS persists the choice; restart
is required because native swap synchronization is selected at startup. Changing
this setting does not enable G-SYNC/FreeSync in the graphics driver or display.
Continuous animation requests repaints without the old 260 ms delay; native
presentation and renderer readiness determine the achievable cadence.


## Experimental GPU Earth rasterization / renderização experimental em GPU

The Earth overview raster can be computed by the `earthcraft-render` `backend`
module with `wgpu` compute shaders (`gpu/`), then read back for the existing
egui view. All GPU globe rendering is experimental on every platform; the CPU
path remains the default and fallback. Set
`EARTHCRAFT_RENDERER=auto` or `EARTHCRAFT_RENDERER=gpu` to enable an experimental GPU attempt.
The Explorer, live meteorological/aircraft layers, moon and custom planet
remain integrated in `apps/earthcraft`. See [GPU_RENDERER.md](GPU_RENDERER.md)
for hardware limits, performance and CPU-only build instructions.

## Sidebar and native menus

The toolbar's sidebar button hides and reopens the sidebar with egui's panel
animation. The sidebar is open at startup; toggling it does not reset layer,
placemark, ruler or section state. Reopening restores its width and scroll state.
Closing or opening sidebar sections must keep the default or manually resized
panel width. Oversized widgets scroll within the panel instead of changing its
width; cursor readouts shrink their font, then reduce decimal precision to keep
both coordinates visible without ellipsis. The comma stays in place when the
pointer leaves the globe. The map uses the space released by the panel. While it animates, frame submission
and publication pause; in-flight jobs can finish without blocking egui. Keep the
previous frame and overlays at their original rectangle, clipped to the available
map area, until a compatible frame for the settled viewport arrives. KML drops
remain available with the sidebar hidden.

macOS uses muda 0.21 (compatible with Rust 1.95) in `apps/earthcraft/src/app/menu`.
Menu objects and handlers stay on the window thread. Callbacks enqueue actions
and wake egui; the app opens dialogs, submits existing KML/screenshot jobs and
closes normally. Screenshot is disabled before a frame exists. Captions follow
language changes and AppKit handles Cmd shortcuts. Edit contains Undo/Redo;
these commands edit focused text or the ruler otherwise. Menu availability
tracks ruler history when text is not focused. Windows and Linux use the
sidebar's file controls; both support Ctrl+O/S/Shift+S/Shift+E and Ctrl+Q
without a native menu bar. All workspace code forbids unsafe Rust.

Smoke check hiding/reopening the sidebar, preserving expanded sections and layer
selections, map navigation while hidden and after resizing, and toggling in both
languages. On macOS also check Open/Save KML, screenshots and Quit through the
native menu. Use isolated settings/cache for these checks.

Ruler editing smoke check: enable the ruler and add several points. Drag a white
point and verify that only that point moves, including after releasing outside
the map. Drag empty map space and verify normal navigation. Hover a point and
press Delete or Backspace to remove only that point; focused text fields must
keep their normal editing behavior. Check both UI languages and Clear all.
