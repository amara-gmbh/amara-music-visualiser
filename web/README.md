# amara music visualiser — browser guide

amara music visualiser runs in a compatible browser and can also be
packaged as a self-contained macOS web app. It shares WGSL world shaders with
the earlier, separate native Rust application, with browser compatibility
adjustments; its renderer, music analysis and scene controller are JavaScript
implementations. The two applications do not have identical features.

The amara wordmark in `web/assets/amara-logo.svg` is the original asset from
the [amara Web Design Playground](https://amarawebdesign.de/spielraum.html?lang=en),
as requested by its owner.

## Run locally

On macOS, from the repository root, start the local server with system-audio
support:

```sh
python3 web/mac-audio-server.py
```

Open <http://127.0.0.1:8768/web/> in a desktop browser with WebGPU available.
The server binds only to this Mac, serves the web app and shared shaders, and
runs for the current terminal session. Stop it with `Ctrl+C`. If port
8768 is already in use, choose another port with `--port` and use that port in
the browser address. The hosted edition is available at
<https://musicvisualiser.amarawebdesign.de/>; the computer-audio bridge is a
local component and is not available directly from the hosted page.

For local-file playback and browser microphone or tab sharing without system audio,
the ordinary `python3 -m http.server 8768 --bind 127.0.0.1` server also works
from the repository root. It does not provide the system-audio bridge.

### Windows computer audio

The visualiser can follow the default Windows output digitally through WASAPI
loopback, without a microphone or the browser's screen-share picker. For the
normal one-click start, double-click **`web/Start amaraMusicVisualiser.vbs`**.
It starts the local web host and audio bridge hidden, then opens the visualiser
in the default browser; no terminal remains open. Node.js must be installed.

For development or troubleshooting, serve the repository on port 8768 and run
the bridge in a second terminal:

```powershell
node web/windows-audio-bridge.mjs --visualiser-port 8768
```

Open <http://127.0.0.1:8768/web/> and click **Listen to computer audio**. The
bridge uses Windows PowerShell and .NET Framework components included with
Windows; it has no npm, Python, driver or virtual-audio-device dependency. The
first start compiles the checked-in C# helper in memory. It captures the default
multimedia render endpoint (falling back to the console endpoint only if needed),
downmixes it to mono and resamples it to 48 kHz before streaming bounded chunks
to the page. Changing the default output device requires stopping and starting
listening again.

If the page reloads while computer audio was selected, it remembers that intent
for the current browser tab. Browser autoplay policy may require one fresh user
gesture; in that case the button and status clearly say **Reconnect computer
audio** instead of silently appearing disconnected. Explicitly stopping or
choosing another source clears the reconnect intent.

The one-click web host and bridge bind only to `127.0.0.1`. The web host serves
only the browser assets and WGSL shaders, rejects other repository paths and
pins the Host header. The bridge accepts only the exact visualiser origin on
`127.0.0.1:8768`, permits one listener and never writes audio to disk. It sends
no audio to the internet. The hidden Node host lasts for the Windows sign-in
session; end its `node.exe` process in Task Manager when you want to shut the
local host down. The manual bridge instead stops with `Ctrl+C`.

## Music and visuals

### Curated journeys

Revision `20261031.11` adds a local **Journey** control. **Surprise Me** keeps
the existing all-world behavior. The curated pools and restrained defaults are:

- **Dreamy:** Fractal Bloom, Glass Tide, Spectral Landscape, Northern Lights,
  Mocca & Cream and Aquarelle; Aquarelle palette, pace 0.55, about 34 seconds.
- **Night Drive:** Deep Flight, Feedback Reverie, Disco Silk, Neon Memory and
  Neon Coast; Cosmic palette, pace 0.90, about 28 seconds.
- **Deep Focus:** Fractal Bloom, Strange Attractor, Spectral Landscape,
  Mandelbrot and Liquid Stained Glass; Mocca & Vanilla, pace 0.38, about 45
  seconds. Mandelbrot retains its 65-second formation window.
- **Dancefloor:** Feedback Reverie, Particle Constellation, Dot Equaliser,
  Disco Silk and Neon Memory; Cosmic, pace 1.15, about 20 seconds.
- **Aquarelle:** the Dreamy world pool with the Aquarelle palette, pace 0.50
  and about 38 seconds.

A journey applies its palette and pace only when selected. Changing palette or
pace afterwards marks that journey customized and is not overwritten. Choosing
a manual world or variation, or loading a profile with a specific world, pauses
the journey; choosing World Auto resumes Surprise Me while preserving current
palette and pace. The status line makes this state explicit.

Music insight changes timing only after a journey's minimum dwell is already
due. Quiet passages can defer a switch by at most 12 seconds, a rising passage
by at most six, and a confident drop can finish an already-due wait. The
existing 7.5-second visual transition remains unchanged, so music cannot cause
rapid-fire scene changes. Only the journey identifier or paused state is stored
in local browser storage; no music insight or audio is persisted.

Revision `20261031.16` keeps those automatic 7.5-second morphs but makes an
explicit World choice immediate. A manual choice during a morph settles to the
visually dominant endpoint and switches directly to a standalone destination
pipeline. Those smaller pipelines warm gradually in the background, avoiding
the much slower first-time source/destination shader-pair compilation. A first
choice made before its warmup finishes can still briefly show **Preparing…**.
Both browser `input` and `change` control events are handled. Journey dwell and
automatic morph progress use their own monotonic wall clock, so the displayed
timing is not stretched by the renderer's deliberately capped animation step.

Revision `20261031.10` derives bounded music insight from the same local analyser
levels used by every input path: file playback, microphone, shared audio,
Windows WASAPI and the Mac bridge. It detects stable onsets, estimates tempo
only after a regular pulse earns confidence, classifies quiet/low/medium/high
energy and rising/steady/falling trend, and emits bounded buildup/drop events.
The live **Music insight** line summarizes this without exposing raw audio.

The first visual mapping is intentionally restrained and uses only existing
controls: buildup can add at most 2.5% pace, 2.5% intensity and 0.03 glow; a
drop adds a smooth decaying maximum of 6% intensity and 0.05 glow. Tempo and
individual onsets do not flash the image or switch worlds. Silence, inactive
input, low tempo confidence and source changes clear the relevant history and
return to the user's exact settings. No samples, tempo history or insight are
written to storage or sent over the network.

Tempo is a heuristic normalized to roughly 60–180 BPM, so half/double-time
ambiguity remains possible, especially with syncopated, ambient or noisy input.
Section and buildup/drop labels describe short-term energy shape rather than
musical genre or semantic song structure.

Click **Listen to computer audio** to follow the computer's current output
without a microphone. On macOS this needs the signed helper at
`outputs/amaraMusicVisualiser.app/Contents/Helpers/amaraAudioCapture` and may
require macOS Audio Capture permission. Start the Mac-audio server above first;
a plain static server cannot capture Mac output. The Mac packaging script
`scripts/build-macos.sh` creates this helper in the app bundle; rebuilding an
ad-hoc signed app can require renewing the macOS audio grant. Use **Stop listening** to
stop capture. The helper's live samples are streamed only to the local page,
analysed in memory, and not recorded or played back by the visualiser. One
browser connection can listen at a time; a second must wait until the first
stops. The macOS server checks the request's local host and origin and doesn't
add cross-origin access. The Windows bridge grants only the exact local
visualiser origin described above.

Choose a local music file to play and analyse it in the browser. The local-track
controls provide play or pause, stop, ten-second skips and a seekable timeline;
the equaliser shows measured frequencies. The app does not upload the selected
music.

Alternatively, choose **Microphone** or **Share audio** and grant the browser
permission. Sharing opens the browser's screen or tab picker: select a source
with audio and enable its audio option if offered. A screen or tab video track
is required by the browser capture API, but the app does not display, save or
send that video. Captured audio is analysed locally and is not played back to
the speakers. Use **Stop listening** to end capture. Audio sharing availability
and whether a selected source provides audio depend on the browser and operating
system. Browser screen/tab sharing is separate from the Mac-audio bridge.

Auto selects from the available worlds without repeating one before a shuffled
cycle is complete; the selector also offers manual choice. When a world is
selected manually, its Variation menu offers all ten named looks. Choosing
another look during a morph queues it until the current morph reaches its
endpoint. The display shows
the active world and variation and, in Auto, progress until the next world or
through a transformation. The equaliser includes peak-hold markers. Use the
independent **Hide controls** and **Hide equaliser** buttons to hide and restore
each panel, and use the full-screen button to expand the view.

When the geometry pass initializes, the menu offers all 17 worlds: Deep Flight,
Fractal Bloom, Glass Tide, Feedback Reverie, Particle Constellation, Strange
Attractor, Spectral Landscape, Dot Equaliser, Water Ripples, Northern Lights,
Mandelbrot, Liquid Stained Glass, Disco Silk, Neon Memory, Neon Coast, Mocca &
Cream and Aquarelle. If
that pass fails but the separate dot pass succeeds, the twelve procedural worlds
and Dot Equaliser remain available; if both geometry passes fail, only the twelve
procedural worlds are available.

## Settings and profiles

Open **Visual settings** to adjust pace, intensity, artistic detail, graphics
quality, palette and the bass, mids and highs mappings. Revision `20261031.9`
adds **Automatic** without changing the default: Balanced remains the normal
baseline. Automatic uses Balanced for lighter settled worlds, selects Efficient
for costly worlds and transitions, and reacts only after sustained bounded GPU
queue-delay samples. Under continuing pressure it can reduce cadence to 20 fps;
after sustained headroom it recovers slowly. Its live status names the active
tier, reason and frame-rate target. It never selects a resolution below the
accepted Efficient artwork floor, stores no hardware identity or GPU percentage,
and keeps calibration in memory for the current page only. Efficient, Balanced
and High Detail remain explicit manual choices and profiles can save Automatic.

Revision `20261031.8`
keeps Efficient at the same high-enough internal resolution for every world:
up to 1152×720 with an 829,440-pixel budget. Light worlds render at 30 fps,
costly worlds and transitions start at 24 fps, and sustained GPU pressure can
step down to 20 fps without reducing resolution. A bounded asynchronous sample of
`device.queue.onSubmittedWorkDone()` drives this governor with cooldown and slow
recovery; drawing never waits for the sample. Balanced remains at up to
1440×900/about 45 fps and High Detail keeps the earlier 1600×1000/60 fps limits.
Graphics quality changes workload only; Detail continues to change the art
itself. Mocca & Vanilla and Aquarelle are
global palettes, available for every world independently of the dedicated Mocca
& Cream and Aquarelle worlds. Under
**Profiles**, name and save the current world and
settings in browser storage, including palette and graphics quality. Select a
saved profile to load, export or delete
it; importing a JSON profile stores it until you explicitly load it. A name
already in use is not silently overwritten. Deleting a profile asks for
confirmation.

Balanced is the normal default and manual baseline. Efficient is the weaker-machine option, but it
still preserves the shared 1152×720 internal artwork budget (aspect-fitted to
the viewport) as its quality floor. The Windows Intel Iris Xe laptop used for
live checks is a very low-GPU-power stress target, not the universal visual or
performance baseline.

A scoped `20261031.8` Windows review at 1815×1039 checked Aquarelle, Particle
Constellation, Strange Attractor, Spectral Landscape, Disco Silk and Neon Coast,
including Particle→Strange and Disco→Neon transitions, with local audio and a
moving equaliser. Fine points, lines, grids and gradients remained intact with
no enlarged square-pixel or blocky-resolution artifacts. Aquarelle's broad
painted shapes and Neon Coast's rectangular buildings and windows are intentional
world geometry, not signs of reduced render resolution. This is visual-quality
acceptance only: settled Neon Coast reached 96% Intel Iris Xe GPU in Efficient,
so the weakest machines can still saturate on the heaviest worlds.

Browser revision `20261031.8` retains the compact 34-pixel action buttons and dropdowns,
thin three-pixel slider tracks with a larger keyboard/focusable control area,
and a four-pixel top inset above the equaliser. Text sizes, spectrum height and
the equaliser's horizontal and bottom padding are unchanged.

The profile JSON uses the Mac version 1 format. Some Mac-only fields are
preserved for export but are not applied by this browser edition; browser Auto
mode is not stored in that native format. Loading a profile never starts or
changes an audio source. Browser storage can be unavailable or cleared by the
browser, so export profiles you want to keep.

### Favorites and visual recipes

Revision `20261031.14` adds one-click local favorites and portable visual
recipes. Open **Favorites & visual recipes**, optionally name the current look,
then choose **Favorite current**. Favorites can be loaded or removed from the
same browser. They use the established profile bounds and enum validation but
store only visual controls: journey state, world/variation, palette, graphics,
pace, intensity, detail and frequency mappings.

**Copy recipe link** and **Copy recipe text** are explicit clipboard actions.
Pasted links or text are validated and shown for review; nothing is applied
until **Review & apply** is chosen. A recipe never contains or changes an audio
source, filename, raw audio, hardware identity, calibration or history. Active,
customized and paused/manual journey state is retained without silently
resuming a journey. Recipe links use the current local URL, so a localhost link
works only on a computer where amaraMusicVisualiser is already running.
Favorites remain in local browser storage and may disappear if that storage is
cleared.

## Focused checks

From the repository root, the dependency-free checks can be run with Node.js:

```sh
node web/tests/audio-capture.test.mjs
node web/tests/mac-audio.test.mjs
node web/tests/mac-audio-worklet.test.mjs
node web/tests/windows-audio-bridge.test.mjs
node web/tests/windows-launcher.test.mjs
node web/tests/music-insight.test.mjs
node web/tests/journeys.test.mjs
node web/tests/recipes.test.mjs
node web/tests/geometry.test.mjs
node web/tests/profiles.test.mjs
node web/tests/profile-adapter.test.mjs
node web/tests/math-journey.test.mjs
node web/tests/asset-revision.test.mjs
node web/choreography.test.mjs
python3 web/tests/mac_audio_server_test.py
```

These checks exercise mocked audio capture, geometry, profiles, visual recipes, scene choreography,
and both loopback bridges with synthetic helpers. They do
not replace browser permission, actual GPU, visual, audio-device or performance
testing. The Windows helper itself must also be exercised on Windows against a
real output device. Browser/device support and visual quality can differ from the Mac
app. The native Rust audio analysis and director, and full native control and
behaviour parity, still need browser adaptations.

When releasing browser updates, increment the matching `?v=` revision on the
stylesheet and module entry in `web/index.html` and on every local module import
in `web/main.js`. Run `node web/tests/asset-revision.test.mjs` to check that
these revisions agree; this prevents an older cached module being paired with
a newer entry point. The renderer and both geometry adapters automatically
pass their module revision to each shared WGSL shader request.
