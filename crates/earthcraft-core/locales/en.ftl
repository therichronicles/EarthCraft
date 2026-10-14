# Embedded UI catalog. Keep keys and variables in sync across locales.

kml-open-dialog-title = Open KML

kml-save-dialog-title = Save KML as

screenshot-empty-error = Nothing to save yet.

kml-drop-type-error = Only .kml files can be dropped here.

globe-placemark-action = 📍 Add placemark here

globe-fly-to-action = ✈ Fly here

globe-ruler-point-action = 📏 Add ruler point

globe-copy-coordinates-action = 📋 Copy coordinates

globe-context-menu-hint = Click on the globe.

app-ready-status = Ready. Drag to rotate, scroll to zoom.

search-empty-status = No results found.

search-pending-status = Searching…

search-input-placeholder = Search a place, address or “lat, lon”

search-submit-action = 🔍 Search

settings-open-tooltip = Settings

navigation-compass-tooltip = Click to align north up

navigation-north-tooltip = North up

navigation-reverse-tooltip = Flip view 180°

navigation-rotate-left-tooltip = Rotate left (hold)

navigation-rotate-right-tooltip = Rotate right (hold)

navigation-pan-north-tooltip = Move north (hold)

navigation-pan-south-tooltip = Move south (hold)

navigation-pan-west-tooltip = Move west (hold)

navigation-pan-east-tooltip = Move east (hold)

navigation-zoom-in-tooltip = Zoom in (hold)

navigation-zoom-out-tooltip = Zoom out (hold)

navigation-zoom-slider-tooltip = Drag to zoom

settings-title = ⚙ Settings

settings-text-section-title = Text size

settings-ui-scale-label = Buttons and panels

settings-map-scale-label = Map text (names, ruler, scale)

settings-folders-section-title = Where to save files

settings-folder-browse-action = 📁 Browse…

settings-screenshot-folder-label = Screenshots (PNG):

settings-screenshot-folder-dialog-title = Screenshots folder

settings-kml-folder-label = KML files:

settings-kml-folder-dialog-title = KML folder

settings-folder-creation-help = Folders are created automatically when saving.

settings-reset-action = Restore defaults

settings-cache-section-title = Map cache
settings-cache-clear-action = Clear on-disk cache
settings-cache-clear-help = Removes downloaded map tiles. They download again when needed.
settings-cache-clearing-label = Clearing cache…
settings-cache-cleared-status = On-disk tile cache cleared.
settings-cache-clear-error = Could not clear on-disk cache: { $error }
settings-cache-disk-usage = { $size } on disk
settings-cache-measuring-label = Measuring…
settings-cache-usage-unavailable = Cache size unavailable

layers-section-title = Layers

wayback-loading-status = ⏳ Loading available dates…

wayback-older-tooltip = Older

wayback-newer-tooltip = Newer

wayback-fallback-help = Not every place has a snapshot on every date; missing areas fall back to the nearest available imagery.

layers-grid-toggle = Latitude/longitude grid

layers-country-borders-toggle = Country borders (worldwide)

layers-state-borders-toggle = States / provinces (worldwide)

layers-place-labels-toggle = Country, state and city names

layers-municipal-borders-toggle = City / municipality borders (Brazil only)

layers-metro-borders-toggle = Metropolitan regions (Brazil only)

boundaries-loading-status = ⏳ Loading borders and names…

layers-atmosphere-toggle = Atmosphere and stars

layers-sunlight-toggle = Real-time sunlight (day/night)

layers-auto-rotation-toggle = Auto-rotate

layers-flights-toggle = Real aircraft (OpenSky + ADSB.lol)
layers-weather-toggle = Live precipitation radar (RainViewer)
layers-clouds-toggle = Satellite cloud map (Matt Eason)
layers-live-overlays-help = Aircraft positions come from live tracking (coverage incomplete); global updates ~17 min and nearby updates ~90 s. Cloud imagery ~3 h; rain radar ~10 min. Internet required.

navigation-section-title = Navigation

navigation-reset-north-action = 🧭 North up

navigation-global-view-action = 🌐 Global view

search-results-label = Search results:

placemarks-section-title = Placemarks

placemarks-name-placeholder = Name

placemarks-add-center-action = ➕ At center

placemarks-fly-to-action = Go

kml-drop-zone-hint = 📥 Drag a .kml file here

kml-save-action = 💾 Save

kml-save-tooltip = Saves into the KML folder (see Settings)

kml-save-as-action = 💾 Save as…

kml-open-action = 📂 Open KML…

ruler-section-title = Ruler

ruler-enable-toggle = Measure distances (click on the globe)

ruler-clear-action = Clear ruler

export-section-title = Export

screenshot-save-action = 📷 Save screenshot (PNG)

settings-folders-action = ⚙ Change folders…

controls-help-toggle = Show controls help

controls-help-text = • Drag (left): rotate the globe
    • Right-drag / Shift+drag: rotate the view
    • Wheel / pinch or +/-: zoom
    • Double-click: fly to the point
    • Arrow keys: move   • N: north up
    • Right-click: context menu

placemarks-fallback-name = Unnamed

provider-satellite-label = Satellite (Esri World Imagery)

provider-wayback-label = Historical imagery (Esri Wayback)

provider-streets-label = Streets (OpenStreetMap)

provider-topography-label = Terrain/Topography (OpenTopoMap)

provider-imagery-attribution = Imagery © Esri, Maxar, Earthstar Geographics

provider-streets-attribution = © OpenStreetMap contributors

# Whole messages: arguments retain the original numeric formatting.

kml-save-success = Saved to { $path }

kml-save-error = Error saving: { $error }

kml-import-success =
    { $count ->
        [1] { $count } placemark imported.
       *[other] { $count } placemarks imported.
    } — { $path }

kml-open-error = Error opening: { $error }

screenshot-save-success = Screenshot saved: { $path }

screenshot-save-error = Error: { $error }

search-navigation-status = Going to { $destination }

search-results-status =
    { $count ->
        [1] { $count } result found.
       *[other] { $count } results found.
    }

search-request-error = Search error: { $error }

wayback-snapshot-attribution = { $attribution } — snapshot { $date }

placemarks-generated-name = Place { $number }

wayback-availability-status = { $count } snapshots available

export-folder-label = Folder: { $path }

ruler-measurement-summary = Points: { $count }   Total: { $distance }

statusbar-tiles-loading = ⏳ loading { $count } tiles

statusbar-camera-center = Center: { $latitude }°, { $longitude }°

statusbar-camera-altitude = Camera altitude: { $distance }


settings-uncap-fps-toggle = Uncap FPS (restart required)
settings-frame-sync-help = By default, presentation synchronizes with the display refresh rate. G-SYNC/FreeSync is configured in your display and graphics driver.

# Explorer / Visualização e novas camadas — conteúdo bilíngue
top-explore-action = Explore +
top-earth = Earth
top-moon = Moon
top-custom-planet = Custom planet
explorer-title = Explore · places, sky and planets
explorer-selected-location = Selected location
explorer-search-place = Search for a place to view information
explorer-latlon = Latitude { $latitude } · Longitude { $longitude }
explorer-center = Center: { $latitude }, { $longitude }
explorer-place-photos = Place photos
explorer-photos-opened = Photo gallery opened in browser
explorer-view-osm = View on OpenStreetMap
explorer-map-opened = Map opened in browser
explorer-photo-copyright = Photos are searched on Wikimedia Commons and copyright belongs to their creators. Place details open in your browser; these are not Google Maps reviews.
explorer-narrated-tour = Narrated tour (TTS)
explorer-tour-default = We are exploring { $place }. Explore the location on the globe and choose points of interest to continue your tour.
explorer-read-aloud = ▶ Read text aloud
explorer-speech-sent = Text sent to speech synthesizer
explorer-globe-layers = Globe layers
explorer-layer-info = These georeferenced layers are drawn on the globe using online data (internet required).
explorer-cloud-credit = Cloud imagery: Matt Eason / Live Cloud Maps
explorer-cloud-source = Live Cloud Maps — author and source code
explorer-cloud-license = Contains modified EUMETSAT data · CC0 1.0
explorer-radar-credit = Radar source: RainViewer
explorer-radar-source = RainViewer weather radar
explorer-radar-explanation = Green/yellow/red indicates approximate rain intensity (radar reflectivity), not infrared satellite temperature. Coverage and freshness vary.
explorer-bodies = Celestial bodies
explorer-earth = 🌍 Earth
explorer-moon = 🌙 Moon
explorer-custom-planet = 🪐 Custom planet
explorer-moon-fixed = The Moon is fixed and cannot be edited. The settings below apply only to the custom planet.
explorer-name = Name:
explorer-image = Image:
explorer-browse = Browse
explorer-display-planet = Display planet image
explorer-planet-displayed = Planet displayed on the globe
explorer-save-planet = Save planet…
explorer-load-planet = Load planet…
explorer-planet-loaded = Planet and sky loaded on the globe
explorer-sky-galaxy = Custom sky and galaxy
explorer-image-path = Image file path
explorer-display-galaxy = Display galaxy
explorer-sky-applied = Sky applied to the globe background
explorer-sky-preview = The custom sky is previewed here; it is not used as the Earth renderer skybox.
live-clouds-failed = Cloud update failed
live-cached-image =  (cached image shown)
live-clouds-ok = Clouds: Matt Eason (3-hour updates)
live-clouds-loading = Clouds: loading satellite imagery…
live-radar-failed = Radar update failed
live-radar-age = Rain radar: observation approximately { $minutes } min old
live-radar-loading = Rain radar: loading RainViewer data…
sidebar-cloud-source = Cloud data: Matt Eason / EUMETSAT
sidebar-radar-source = Radar data: RainViewer
live-cloud-credit = Matt Eason · Live Cloud Maps · Contains modified EUMETSAT data
live-radar-credit = Precipitation radar: RainViewer (rainviewer.com)

# Native desktop menus
menu-file-title = File
menu-kml-open = Open KML…
menu-kml-save = Save KML
menu-kml-save-as = Save KML As…
menu-screenshot-save = Save Screenshot
menu-quit-action = Quit EarthCraft

sidebar-hide-tooltip = Hide sidebar
sidebar-show-tooltip = Show sidebar

settings-distance-units-label = Distance units
settings-units-system = System locale
settings-units-metric = Kilometers / meters
settings-units-imperial = Miles / feet

ruler-unit-automatic = Automatic
ruler-unit-centimeters = Centimeters
ruler-unit-meters = Meters
ruler-unit-kilometers = Kilometers
ruler-unit-inches = Inches
ruler-unit-feet = Feet
ruler-unit-yards = Yards
ruler-unit-miles = Miles
ruler-unit-nautical-miles = Nautical miles
ruler-unit-smoots = Smoots
ruler-unit-degrees = Degrees
ruler-unit-arcseconds = Arcseconds
ruler-unit-football-fields = Football fields
ruler-unit-american-football-fields = American football fields
ruler-units-label = Units
ruler-distance-smoots = { $value } smoots
ruler-distance-football-fields = { $value } football fields
ruler-unit-automatic-help = Use the distance units selected in Settings.
ruler-unit-centimeters-help = 1 centimeter = 0.01 meters.
ruler-unit-meters-help = 1 meter = 100 centimeters.
ruler-unit-kilometers-help = 1 kilometer = 1,000 meters.
ruler-unit-inches-help = 1 inch = 2.54 centimeters.
ruler-unit-feet-help = 1 foot = 12 inches.
ruler-unit-yards-help = 1 yard = 3 feet.
ruler-unit-miles-help = 1 mile = 1,760 yards.
ruler-unit-nautical-miles-help = 1 nautical mile = 1,852 meters.
ruler-unit-smoots-help = 1 smoot = 67 inches.
ruler-unit-degrees-help = Central angle along the measured path on the globe.
ruler-unit-arcseconds-help = 1 arcsecond = 1/3,600 of a degree of central angle.
ruler-unit-football-fields-help = One American football field is 100 yards, excluding end zones.

altitude-edit-tooltip = Set an exact camera altitude
altitude-edit-title = Camera altitude
altitude-apply-action = Apply
altitude-cancel-action = Cancel
altitude-input-error = Enter a number between { $minimum } and { $maximum } { $unit }.

settings-hide-navigation-controls-toggle = Hide navigation controls

settings-close-action = Close

ruler-point-edit-help = Alt-click: new path · Drag: move · Hover + Delete: remove
ruler-point-edit-help-macos = Option-click: new path · Drag: move · Hover + Delete: remove
compass-north = N
compass-northeast = NE
compass-east = E
compass-southeast = SE
compass-south = S
compass-southwest = SW
compass-west = W
compass-northwest = NW
compass-heading = { $degrees }° { $direction }
compass-heading-tooltip = Direction at the top of the map, clockwise from true north. Click to reset north.
layers-grid-numbers-toggle = Show numbers
ruler-bearing-label = Last segment bearing: { $bearing }
ruler-bearing-tooltip = Initial great-circle bearing from the previous point to the last point, clockwise from true north.
grid-coordinate-label = { $degrees }° { $direction }

menu-edit-title = Edit
edit-undo-action = Undo
edit-redo-action = Redo

ruler-points-edit-action = Edit coordinates…
ruler-points-title = Ruler coordinates
ruler-path-heading = Path { $number }
ruler-point-row = { $number }: { $latitude }°, { $longitude }°
ruler-point-edit-mode = Edit selected point
ruler-point-add-mode = Add a point
ruler-point-latitude = Latitude (°)
ruler-point-longitude = Longitude (°)
ruler-point-new-path-toggle = Start a separate path
ruler-point-input-help = Decimal degrees. Negative latitude is south; negative longitude is west.
ruler-point-apply-action = Apply
ruler-point-add-action = Add point
ruler-point-new-action = New point
ruler-point-close-action = Close
