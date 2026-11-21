# Test fixtures

`pointer_scenarios.json` holds observations of how the reference Classic app
(GeoGebra's web build, version 5.4.936.0, driven by real pointer, touch and
keyboard input in headless Chromium at 1280 x 800, standard view, English UI)
reacts to pointer input: drags, presses, pans, zooms and tool clicks. Each
scenario has the script typed before the gesture, the input events (world
coordinates in the standard view, times in milliseconds) and the measured
outcome (coordinates, the view origin and scale, selection, created objects,
label offsets, undo step counts).

These are observations of UI behaviour, not golden data for the kernel. They
contain no captions, screenshots or other material of the reference app:
scripts, input events, numbers and object labels only. Algebra-view strings
were replaced by numbers (`line_implicit`: the coefficients of `a x + b y = c`)
or by `unchanged` (the object keeps the value the script gave it).

`replay.rs` runs every scenario through the headless window and reports for
each one whether it passes, fails, or is not applicable yet (it needs a
later part of the pointer work).

The `source` field identifies the reference build and capture method. The
tool recordings use the Classic reference probe
`classic6-reference/headless/lib.mjs` to send real clicks and collect object
definitions, values, selection events and undo depth. Where
`reference_axes_visible` is false, the probe hid the axes to isolate the
intended hits. Area observations retain the recorded text and anchor
objects. Replay compares the supported numeric measurement under the
controller's area label.
