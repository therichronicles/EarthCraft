[**English**](README.md) | [Português (Brasil)](README.pt-BR.md)

# EarthCraft 🌍

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade.svg)](https://remade.wiki/projects/earthcraft/)

A 3D globe in the style of Google Earth, written in pure Rust (egui + CPU ray-casting globe rendering, with optional, experimental GPU acceleration). Runs on Windows, macOS and Linux with no system dependencies beyond a graphics driver.

### Build and run
Requires Rust 1.95+ (https://rustup.rs):

    cargo run --release

Linux (if something is missing):  `sudo apt install libxkbcommon-dev libwayland-dev libx11-dev` / `sudo apt install build-essential pkg-config libatk1.0-dev libgtk-3-dev`

An internet connection is required to download map tiles.

**macOS downloads (Apple Silicon):** Open the DMG and drag EarthCraft to Applications. The app
is not notarized, so Gatekeeper may block the first launch. Try opening it once,
dismiss the warning, then go to **System Settings → Privacy & Security → Open
Anyway** for EarthCraft and confirm **Open**. See [Apple's instructions](https://support.apple.com/en-us/102445).

### Features

- 3D globe with orbit, zoom, view rotation, bottom-bar heading and a navigation compass; navigation controls can be hidden in Settings
- Satellite imagery (Esri), streets (OSM) and topography (OpenTopoMap), with multi-level tile zoom and on-disk caching
- Background satellite overview loading for the whole globe at the initial distance; detailed tiles load as you zoom in
- Clear downloaded map tiles from disk through Settings
- Place and address search (Nominatim), plus direct "lat, lon" coordinate input
- Animated fly-to (double-click, search or list)
- Placemarks with KML import/export
- Geodesic distance ruler (great-circle), with exact decimal-degree point entry/editing, undo/redo, separate paths (Alt-click), selectable metric, imperial, nautical, angular and novelty units and initial bearing for the last segment
- Lat/lon grid with optional coordinate labels, atmosphere, stars, real-time sunlight (day/night)
- Scale bar, cursor coordinates, PNG screenshot, context menu
- Background CPU rendering; experimental GPU globe renderer can be enabled via `EARTHCRAFT_RENDERER=auto` or `EARTHCRAFT_RENDERER=gpu` (with CPU fallback). See [experimental GPU rendering](docs/GPU_RENDERER.md).

### Controls
Drag: rotate globe · Right-drag / Shift+drag: rotate view · Wheel / +/-: zoom · Double-click: fly to point · Arrow keys: move · N: north up

Click **Camera altitude** in the bottom bar to enter an exact altitude.

Use **☰** in the toolbar to hide or reopen the sidebar and give the map more room.

On macOS, the native **File** menu opens/saves KML and saves PNG screenshots. Use Cmd+O to open, Cmd+S to save, Cmd+Shift+S for Save As, and Cmd+Shift+E for a screenshot. Screenshots use the folder selected in Settings. Quit is in the application menu. Windows and Linux use the sidebar's file controls and the same shortcuts with Ctrl instead of Cmd; Ctrl+Q quits.

### Not implemented (would require proprietary data/infrastructure)

Real 3D terrain, 3D buildings, Street View, Google Earth Studio, geotagged photos, narrated tours, icons
Tiles come from third-party services: please respect their terms of use (OSM limits heavy usage).

### Development

The desktop app lives in `apps/earthcraft`, with reusable libraries in `crates/`.
See the [architecture](docs/ARCHITECTURE.md), [contributing guide](docs/CONTRIBUTING.md),
and [agent rules](AGENTS.md) for module ownership, checks, and how to extend the project.


### Explorer add-on (October 2026) / Módulo Explorar

Open **Explorar +** in the top bar for: coordinates and external OSM details of
search results; Wikimedia Commons photo search; editable narrated tours using
the operating system TTS (Linux requires `espeak`); external real-world
weather/cloud/ocean/aircraft/satellite data websites; links to NASA Moon and
Mars resources; local PNG/JPEG planet image preview; save and reload custom
`.earthplanet` manifests (keep the accompanying `*-texture.png/jpg` file in
the same directory); and preview custom sky/galaxy images.

**Limitations:** This project does not provide full Google Maps place cards,
licensed photos embedded in the application, actual 3D buildings or detailed
terrain. Live flight and weather layers depend on external providers. The
Moon and custom planet use egui meshes rather than GPU compute rendering.
The Earth globe also supports an experimental GPU overview renderer; Explorer planets are still drawn by egui. The explorer's
websites require an internet connection. This code was not compiled or
executed in the delivery environment (Rust toolchain unavailable).

### Explorer language / Idioma do Explorar
The Explorer panel and toolbar labels follow the existing top-right language selector (English / Português (Brasil)). User-authored narration is preserved when changing language.
O painel Explorar e a barra superior acompanham o seletor de idioma no canto superior direito. Textos de narração editados pelo usuário não são traduzidos automaticamente.
