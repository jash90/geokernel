# Interaction parity observations

These results update the Classic 6 interaction spec by rule ID. The [recording
instructions](../tools/ggb-harness/README.md#interaction-parity-recordings)
and the [capture header](../crates/geo-mcp/tests/fixtures/parity_scenario_capture.json)
identify the reference revision, harness revision and Linux x86_64 command.
These cases use the shared Java controller. Platform event delivery and
painted platform widgets require separate evidence.

## Previously unconfirmed rules

| Rule | Recording | Result |
| --- | --- | --- |
| A | `controller_drag_time_gate` | A move at 80 ms leaves the point in place; at 150 ms it moves. This confirms the controller gate. Chromium frame-aligned dispatch and the observed 85–91 ms remain unmeasured. |
| A6 | `rapid_second_press_on_point` | The second press 60 ms after release sets the double-click flag and opens the point's redefine dialog on release. A press after 560 ms leaves the flag clear. The controller uses the flag to suppress a second move press and dispatch double-click handling. |
| B4 | `press_on_polygon_edge_does_nothing` | Refuted in the Java controller. It selects side `c`, moves its two endpoints and stores an undo step. The older web observation remains a separate measurement. |
| B8 | `hit_radius_mouse_and_touch` | At 20 px from the point, the mouse misses and the touch hits. Native egui attribution is outside this reference test. |
| C3 | `dependent_line_snap` | Confirmed. The first free point lands at (2, 2); the second moves by the same displacement to (5, 3). |
| C11 | `grid_steps_during_zoom` | Records the grid distances at each scale. The app and the reference use the same major step at the sampled scales. |
| D6 | `resize_after_view_reload` | GeoGebra reloads the panned and zoomed view. The desktop also saves and imports the view in a .ggb archive. The plain workbench script snapshot omits the camera. |
| D9 | `zoom_rectangle_near_view_aspect`, `zoom_in_tool_rectangle_fills_canvas` | The near-aspect drag expands a 300 × 240 rectangle to 300 × 251. The skewed drag keeps independent dimensions. The snap is conditional on the aspect ratio. |
| D11 | `resize_after_view_reload` | The headless reload retains the origin and scale, and resizing keeps the origin. The desktop and web resize callbacks themselves are outside this recording. |
| D13 | `pan_near_axis_inside_lock_cone`, `pan_off_axis` | A 12 px initial move near the horizontal axis locks the vertical origin. A first move already beyond 40 px pans on both axes. |
| E7 | `segment_preview_endpoint` | The preview and the finished segment have the same RGB values, line type and thickness. The probe reads the drawable properties without comparing platform antialiasing. |
| E6, E10, C5 | `segment_preview_hits_existing_point`, `segment_tool_twice_stays_selected`, `drag_point_on_line_slides` | The preview ends on the existing point despite an offset pointer. Segment remains active after both completions. The constrained point remains on its line. |
| E11, E12 | `delete_tool_stroke_erases_crossed_object`, `area_tool_on_polygon`, `circle_radius_dialog_cancel` | The eraser takes one undo step. Area adds a text label and region point in one step. Cancel leaves the radius dialog's newly placed point and creates no circle. The headless dialog cancels at the callback boundary. |
| F5, G8 | `enter_on_empty_input_row` | The input preview normalizes the empty string to empty. The Java probe adds no objects or undo step. Both platform input bars return before evaluation for this value. The earlier web observation supplies the keyboard-state assertion. |
| F3 | `wheel_checkpoint_at_next_press`, `wheel_checkpoint_at_exit`, `zoom_tool_checkpoint_on_leave` | Confirmed. The wheel keeps zero checkpoints until the next press or mouse exit. Zoom-tool click keeps zero until the tool is left. |
| G1 | `algebra_description_separators` | The default algebra descriptions print `A = (1, 1)`, `a = 3`, `f: y = 1` and `g(x) = x²`. The function's declaration uses assignment syntax. |
| G6 | `select_point_for_algebra_highlight` | The controller selects `A`. Cell highlighting is a renderer decision and remains unverified by this harness. |
| G7 | `context_menu_routes_object_or_canvas` | Right-click on empty space requests the drawing-pad menu. Right-click on `A` requests the object menu for `A`. The menu item builders sit beyond these GUI hooks. |
| H4 | `pinch_sub_threshold_step` | A 2 px change does not zoom. The next move, from a starting span of 200 px to 300 px, reaches 1.5×. This probe refutes losing the first sub-threshold step in the shared controller. The old 1.485× platform measurement remains unexplained. |
| H5 | `two_fingers_scale_x_axis`, `two_fingers_scale_circle`, `two_fingers_move_line` | The gesture controller can scale x alone, increase a two-point circle's radius from 2 to 3, or move both defining points of a line. |
| W1 | `point_visibility_with_automatic_labeling` | Automatic labeling shows the label with algebra open and hides it with algebra closed. The particular saved preference in the older desktop measurement remains unverified. |
| W4, W5 | `drag_release_outside_canvas` | The controller moves the point to the view edge when it is supplied with an outside move and records zero undo steps after the outside release. Swing delivery of those events remains unverified. |

## Native-app design cautions

1. Fixed zoom per wheel event makes the speed depend on a device's event rate.
2. Pinch dead bands make the view lag behind nearby finger positions.
3. Moving without a separate undo checkpoint can fold a small edit into an earlier action.
4. Extreme zoom scales exceed the useful precision of the renderer and geometry tolerances.
5. Lazy wheel checkpoints make the Undo result depend on which control the user clicks next.
6. A time gate can suppress the start of a quick flick.
7. Axis locking can discard an intended small amount of diagonal pan.
8. A visual response helps explain why a selected dependent object refuses a drag.
9. The recorded polygon edge actually hits a side, so a hard-coded dead zone would discard a valid target.
