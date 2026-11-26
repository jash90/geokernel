package geokernel.harness;

import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.geogebra.common.euclidian.EuclidianConstants;
import org.geogebra.common.euclidian.EuclidianController;
import org.geogebra.common.euclidian.EuclidianView;
import org.geogebra.common.euclidian.GetViewId;
import org.geogebra.common.euclidian.controller.MouseTouchGestureController;
import org.geogebra.common.euclidian.draw.DrawSegment;
import org.geogebra.common.euclidian.event.PointerEventType;
import org.geogebra.common.awt.GPoint;
import org.geogebra.common.awt.AwtFactory;
import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.gui.Layout;
import org.geogebra.common.gui.dialog.ButtonDialogModel;
import org.geogebra.common.gui.dialog.TextInputDialog;
import org.geogebra.common.gui.dialog.handler.TextBuilder;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.DialogManagerNoGui;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.gui.layout.DockManager;
import org.geogebra.common.gui.layout.DockPanel;
import org.geogebra.common.io.layout.Perspective;
import org.geogebra.common.kernel.ModeSetter;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoInputBox;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.kernel.kernelND.GeoPointND;
import org.geogebra.common.kernel.kernelND.GeoSegmentND;
import org.geogebra.common.main.DialogManager;
import org.geogebra.common.main.GuiManagerInterface;
import org.geogebra.common.main.settings.LabelVisibility;
import org.geogebra.common.main.settings.LayoutSettings;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.test.TestErrorHandler;
import org.geogebra.test.TestEvent;

/** Records the controller's construction state after each pointer gesture. */
public final class ToolPointer {
    private static final int SELECTION_LABEL_WORK = 65_536;

    private ToolPointer() {
    }

    public static void main(String[] args) throws Exception {
        boolean parity = "--parity".equals(args[0]);
        if (parity) {
            checkSelectionBudget();
        }
        var captures = new ArrayList<Object>();
        for (String input : parity ? List.of(args[1]) : List.of(args[0], args[1])) {
            var scenarios = (List<?>) ((Map<?, ?>) Json.parse(Files.readString(Path.of(input))))
                    .get("scenarios");
            for (Object entry : scenarios) {
                var scenario = (Map<?, ?>) entry;
                String name = (String) scenario.get("name");
                if (parity || (name.startsWith("tool_") && (input.equals(args[0]) || List.of(
                        "tool_input_box_places_empty_widget", "tool_create_list_from_rectangle",
                        "tool_text_places_literal", "tool_locus_uses_path_point",
                        "tool_pen_records_drag").contains(name)))) {
                    captures.add(record(scenario, parity));
                }
            }
        }
        var output = new LinkedHashMap<String, Object>();
        output.put("upstream", args[3]);
        if (parity) {
            output.put("generator", "ggb-harness");
            output.put("harness_revision_sha256", args[4]);
            output.put("command", "GEOGEBRA_SRC=<checkout> tools/ggb-harness/linux-x86_64.sh golden parity-scenarios");
        }
        output.put("scenarios", captures);
        Files.writeString(Path.of(args[2]), Json.write(output));
    }

    private static int chargeSelection(int remaining, int count) {
        if (count > remaining) {
            throw new IllegalArgumentException("Selection recording exceeds " + SELECTION_LABEL_WORK + " labels");
        }
        return remaining - count;
    }

    private static void checkSelectionBudget() {
        int remaining = SELECTION_LABEL_WORK;
        for (int i = 0; i < 256; i++) {
            remaining = chargeSelection(remaining, 256);
        }
        if (remaining != 0) {
            throw new AssertionError("Selection allowance was not consumed");
        }
        try {
            chargeSelection(remaining, 1);
        } catch (IllegalArgumentException expected) {
            return;
        }
        throw new AssertionError("Selection recording exceeded its allowance");
    }

    private static Map<String, Object> record(Map<?, ?> input, boolean parity) throws Exception {
        var app = new PointerApp(parity);
        var view = app.getActiveEuclidianView();
        app.setAppletFlag(false);
        if ("context_menu_routes_object_or_canvas".equals(input.get("name"))) {
            app.enableGuiProbe();
        }
        if (parity) {
            view.setPreferredSize(AwtFactory.getPrototype().newDimension(892, 747));
        }
        view.setCoordSystem(446, 373.5, 50, 50);
        var controller = view.getEuclidianController();
        var touch = new MouseTouchGestureController(app, controller);
        boolean timeGate = "controller_drag_time_gate".equals(input.get("name"));
        if (!parity) {
            controller.setDraggingDelay(0);
        }
        app.setMode(((Number) input.get("tool_mode")).intValue());
        for (Object line : (List<?>) input.get("initial_script")) {
            if (app.getKernel().getAlgebraProcessor().processAlgebraCommand((String) line, false)
                    == null) {
                throw new IllegalArgumentException("Could not run " + line);
            }
        }
        app.getKernel().setUndoActive(true);
        app.getKernel().initUndoInfo();
        int baseline = app.getUndoManager().getHistorySize();

        var result = new LinkedHashMap<String, Object>();
        for (String key : List.of("name", "tool_mode", "initial_script", "events")) {
            result.put(key, input.get(key));
        }
        var observe = (List<?>) input.get("observe");
        boolean area = parity && app.getMode() == EuclidianConstants.MODE_AREA;
        var initial = snapshot(app, observe, area);
        result.put("initial", initial);
        var frames = new ArrayList<Object>();
        var interaction = new ArrayList<Object>();
        int selectionWork = SELECTION_LABEL_WORK;
        boolean dragging = false;
        long pressedAt = 0;
        String answer = "";
        boolean inputRow = false;
        var metadata = new LinkedHashMap<String, Object>();
        for (Object item : (List<?>) input.get("events")) {
            var event = (Map<?, ?>) item;
            String type = (String) event.get("type");
            var pos = (List<?>) event.get("pos");
            TestEvent pointer = null;
            if (pos != null) {
                double x = number(pos.get(0)), y = number(pos.get(1));
                pointer = parity
                        ? new TestEvent((int) Math.round(446 + 50 * x),
                                (int) Math.round(373.5 - 50 * y), PointerEventType.MOUSE,
                                Boolean.TRUE.equals(event.get("right_click")))
                        : new TestEvent(view.toScreenCoordX(x), view.toScreenCoordY(y));
            }
            long time = ((Number) event.get("t")).longValue();
            if (parity && dragging && "mouse_move".equals(type)) {
                long elapsed = event.get("since_press_ms") instanceof Number supplied
                        ? supplied.longValue() : time - pressedAt;
                controller.setLastMousePressedTime(System.currentTimeMillis() - elapsed);
            }
            if (event.get("since_release_ms") instanceof Number elapsed) {
                controllerField("lastPointerRelease").setLong(controller,
                        System.currentTimeMillis() - elapsed.longValue());
            }
            switch (type) {
                case "mouse_down" -> {
                    controller.wrapMouseMoved(pointer);
                    controller.wrapMousePressed(pointer);
                    dragging = true;
                    pressedAt = time;
                }
                case "mouse_move" -> {
                    if (dragging) {
                        controller.wrapMouseDragged(pointer, true);
                    } else {
                        controller.wrapMouseMoved(pointer);
                    }
                }
                case "mouse_up" -> {
                    controller.wrapMouseReleased(pointer);
                    dragging = false;
                }
                case "text" -> answer = (String) event.get("text");
                case "command" -> {
                    if (app.getKernel().getAlgebraProcessor()
                            .processAlgebraCommand((String) event.get("input"), true) == null) {
                        throw new IllegalArgumentException("Could not run " + event.get("input"));
                    }
                }
                case "focus_input_row" -> inputRow = true;
                case "key" -> {
                    if ("Enter".equals(event.get("key"))) {
                        if (inputRow) {
                            // Both platform input bars call this shared helper before their empty-input branch.
                            // The headless view has no platform input field or virtual keyboard.
                            var preview = app.getKernel().getInputPreviewHelper();
                            metadata.put("normalized_input", preview.getInput(answer));
                            preview.updatePreviewFromInputBar("", app.getDefaultErrorHandler());
                        } else {
                            app.submit(answer);
                        }
                    } else if ("Escape".equals(event.get("key")) && app.radiusDialogRequested()) {
                        // Only the request reaches AppCommon. Escape belongs to the native dialog.
                        metadata.put("unhandled_platform_key", "Escape");
                    } else {
                        throw new IllegalArgumentException("Unsupported dialog key " + event);
                    }
                }
                case "wheel" -> controller.wrapMouseWheelMoved(pointer.getX(), pointer.getY(),
                        number(event.get("deltaY")), false, false);
                case "mouse_exit" -> controller.wrapMouseExited(null);
                case "select_tool" -> view.setMode(((Number) event.get("mode")).intValue(), ModeSetter.TOOLBAR);
                case "reload_view" -> app.setXML(app.getXML(), true);
                case "resize_canvas" -> {
                    view.setPreferredSize(AwtFactory.getPrototype().newDimension(
                            (int) number(event.get("width")), (int) number(event.get("height"))));
                    view.updateSize();
                }
                case "algebra_visibility" -> {
                    app.enableGuiProbe();
                    app.algebraVisible = Boolean.TRUE.equals(event.get("visible"));
                    app.getSettings().getLabelSettings().setLabelVisibility(LabelVisibility.Automatic);
                }
                case "hit_test" -> {
                    var source = "touch".equals(event.get("source"))
                            ? PointerEventType.TOUCH : PointerEventType.MOUSE;
                    view.setHits(new GPoint(pointer.getX(), pointer.getY()), source);
                    metadata.put(source.name(), view.getHits().stream().map(GeoElement::getLabelSimple).toList());
                }
                case "algebra_descriptions" -> {
                    var descriptions = new LinkedHashMap<String, String>();
                    for (var geo : app.getKernel().getConstruction().getGeoSetConstructionOrder()) {
                        descriptions.put(geo.getLabelSimple(), geo.getAlgebraDescriptionDefault());
                    }
                    metadata.put("algebra_descriptions", descriptions);
                }
                case "pinch_start", "pinch_move", "touch_gesture_start", "touch_gesture_move" -> {
                    var fingers = (List<?>) event.get("fingers");
                    var a = (List<?>) fingers.get(0);
                    var b = (List<?>) fingers.get(1);
                    var coordinates = new double[] {number(a.get(0)), number(a.get(1)),
                            number(b.get(0)), number(b.get(1))};
                    double x1 = coordinates[0], y1 = coordinates[1];
                    double x2 = coordinates[2], y2 = coordinates[3];
                    switch (type) {
                        case "pinch_start" -> controller.twoTouchStart(x1, y1, x2, y2);
                        case "pinch_move" -> controller.twoTouchMove(x1, y1, x2, y2);
                        case "touch_gesture_start" -> touch.twoTouchStart(x1, y1, x2, y2);
                        case "touch_gesture_move" -> touch.twoTouchMove(x1, y1, x2, y2);
                        default -> throw new AssertionError(type);
                    }
                }
                case "pinch_end" -> controller.twoTouchEnd();
                default -> throw new IllegalArgumentException("Unsupported pointer event " + type);
            }
            if (parity) {
                var selection = app.getSelectionManager().getSelectedGeos();
                selectionWork = chargeSelection(selectionWork, selection.size());
                var state = dialog("t", event.get("t"), "view",
                        viewCoordinates(view), "grid_step", List.of(view.getGridDistances(0), view.getGridDistances(1)),
                        "mode", controller.getMode(),
                        "selection", selection.stream()
                                .map(GeoElement::getLabelSimple).toList(),
                        "undo_steps_added", app.getUndoManager().getHistorySize() - baseline);
                if (timeGate) {
                    var point = (GeoPointND) app.getKernel().lookupLabel("A");
                    state.put("point", List.of(point.getInhomX(), point.getInhomY()));
                }
                if (event.containsKey("since_release_ms")) {
                    state.put("double_click_started", controllerField("doubleClickStarted").getBoolean(controller));
                }
                var rectangle = view.getSelectionRectangle();
                if (rectangle != null) {
                    state.put("rectangle", List.of(rectangle.getX(), rectangle.getY(),
                            rectangle.getWidth(), rectangle.getHeight()));
                }
                if (view.getPreviewDrawable() instanceof DrawSegment segment && "mouse_move".equals(type)) {
                    var preview = segment.getLine();
                    state.put("segment_preview", List.of(preview.getX1(), preview.getY1(),
                            preview.getX2(), preview.getY2()));
                    state.put("preview_style", strokeStyle(segment.getGeoElement()));
                }
                interaction.add(state);
            }
            if ("mouse_up".equals(type) || "key".equals(type)) {
                var frame = new LinkedHashMap<String, Object>();
                frame.put("t", event.get("t"));
                frame.put("objects", snapshot(app, observe, area));
                frames.add(frame);
            }
        }
        result.put("frames", frames);
        result.put("dialogs", app.dialogs);
        if (parity) {
            result.put("interaction", interaction);
            if ("point_visibility_with_automatic_labeling".equals(input.get("name"))) {
                metadata.put("visible_algebra_label", app.getKernel().lookupLabel("A").isLabelVisible());
                metadata.put("hidden_algebra_label", app.getKernel().lookupLabel("B").isLabelVisible());
            }
            if ("segment_preview_endpoint".equals(input.get("name"))) {
                metadata.put("created_segment_style", strokeStyle(app.getKernel().lookupLabel("f")));
            }
            result.put("metadata", metadata);
            result.put("settled", snapshot(app, observe, area));
        }
        var undo = new ArrayList<Object>();
        if (parity) {
            String saved = area && observe == null ? app.getXML() : null;
            var undoViews = new ArrayList<Object>();
            while (app.getUndoManager().getHistorySize() > baseline) {
                int depth = app.getUndoManager().getHistorySize();
                app.getKernel().undo();
                if (app.getUndoManager().getHistorySize() != depth - 1) {
                    throw new IllegalStateException("Undo did not remove exactly one checkpoint");
                }
                undo.add(snapshot(app, observe, area));
                undoViews.add(viewCoordinates(view));
            }
            result.put("undo_views", undoViews);
            if (saved != null) {
                app.setXML(saved, true);
                result.put("reloaded", snapshot(app, observe, area));
            }
        } else {
            for (int i = 0; i < 4 && !snapshot(app, observe, area).equals(initial); i++) {
                app.getKernel().undo();
                undo.add(snapshot(app, observe, area));
            }
        }
        result.put("undo", undo);
        return result;
    }

    private static Map<String, Object> snapshot(PointerApp app, List<?> observe, boolean area) {
        var result = new LinkedHashMap<String, Object>();
        var objects = app.getKernel().getConstruction().getGeoSetConstructionOrder();
        if (observe != null) {
            result.put("$object_count", objects.size());
        }
        for (GeoElement geo : objects) {
            if (observe != null && !observe.contains(geo.getLabelSimple())) {
                continue;
            }
            var state = new LinkedHashMap<>(Session.describe(geo, Session.typeOf(geo)));
            if (app.requireSubmission) {
                if (geo instanceof GeoText text) {
                    var anchor = text.getStartPoint();
                    if (anchor != null && anchor.isLabelSet()) {
                        state.put("anchor", anchor.getLabelSimple());
                    } else if (text.isAbsoluteScreenLocActive()) {
                        state.put("screen_position", List.of(text.getAbsoluteScreenLocX(), text.getAbsoluteScreenLocY()));
                    } else if (anchor != null) {
                        state.put("world_position", List.of(anchor.getInhomX(), anchor.getInhomY()));
                    }
                    if (area) {
                        // Keep the measured subject and value independently of the locale's wording.
                        String caption = text.getTextString();
                        int equals = caption.lastIndexOf(" = ");
                        if (equals >= 0) {
                            String subject = caption.substring(0, equals).stripTrailing();
                            state.put("measure_subject", subject.substring(subject.lastIndexOf(' ') + 1));
                            state.put("measure_value", caption.substring(equals + 3));
                        }
                        state.remove("text");
                        var background = text.getBackgroundColor();
                        state.put("background", background == null ? null
                                : List.of(background.getRed(), background.getGreen(), background.getBlue()));
                    }
                } else if (geo instanceof GeoInputBox box) {
                    state.put("screen_position", List.of(box.getAbsoluteScreenLocX(), box.getAbsoluteScreenLocY()));
                }
            }
            if (geo instanceof GeoPointND point) {
                if (area) {
                    state.put("visible", geo.isEuclidianVisible());
                    state.put("auxiliary", geo.isAuxiliaryObject());
                    state.put("label_visible", geo.isLabelVisible());
                }
                state.put("on_path", point.isPointOnPath());
                state.put("in_region", point.isPointInRegion());
            }
            result.put(geo.getLabelSimple(), state);
        }
        return result;
    }

    private static List<Double> viewCoordinates(EuclidianView view) {
        return List.of(view.getXZero(), view.getYZero(), view.getXscale(), view.getYscale());
    }

    private static Field controllerField(String name) throws ReflectiveOperationException {
        Field field = EuclidianController.class.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static List<Integer> strokeStyle(GeoElement geo) {
        var color = geo.getObjectColor();
        return List.of(color.getRed(), color.getGreen(), color.getBlue(),
                geo.getLineType(), geo.getLineThickness());
    }

    private static double number(Object value) {
        return ((Number) value).doubleValue();
    }

    private static Map<String, Object> dialog(Object... fields) {
        var result = new LinkedHashMap<String, Object>();
        for (int i = 0; i < fields.length; i += 2) {
            result.put((String) fields[i], fields[i + 1]);
        }
        return result;
    }

    private static LocalizationCommon localization() {
        var loc = new LocalizationCommon(2);
        loc.setResourceBundleControl(new Utf8Control());
        return loc;
    }

    /** One fixed graphics pane for the GUI-boundary probes. */
    private static final class ProbeLayout extends Layout {
        private final DockManager dockManager = new DockManager() {
            @Override public GetViewId getFocusedEuclidianPanel() { return null; }
            @Override public boolean setFocusedPanel(int panel) { return false; }
            @Override public void unRegisterPanel(DockPanel panel) { }
            @Override public DockPanel getPanel(int viewId) { return null; }
            @Override public int getNumberOfOpenViews() { return 1; }
            @Override public int getFocusedViewId() { return AppCommon.VIEW_EUCLIDIAN; }
            @Override public void setLabels() { }
        };

        @Override public DockManager getDockManager() { return dockManager; }
        @Override public Perspective createPerspective() { return null; }
        @Override public boolean applyPerspective(Perspective perspective) { return false; }
        @Override public boolean isOnlyVisible(int viewId) { return viewId == AppCommon.VIEW_EUCLIDIAN; }
        @Override public void settingsChanged(LayoutSettings settings) { }
    }

    private static final class PointerApp extends AppCommon {
        private final List<Object> dialogs = new ArrayList<>();
        private GuiManagerInterface guiProbe;
        private boolean algebraVisible;
        private GeoPointND start;
        private GeoPointND[] points;
        private GeoSegmentND[] segments;

        private TextBuilder textBuilder;
        private ButtonDialogModel inputBox;
        private final boolean requireSubmission;

        PointerApp(boolean submitTextAndInputBox) throws ReflectiveOperationException {
            super(localization(), new AwtFactoryCommon(), new AppConfigDefault());
            requireSubmission = submitTextAndInputBox;
            Field dialog = AppCommon.class.getDeclaredField("dialogManager");
            dialog.setAccessible(true);
            dialog.set(this, new DialogManagerNoGui(this, new String[0]) {
                @Override
                public void showNumberInputDialogSegmentFixed(String title, GeoPointND point) {
                    start = point;
                    dialogs.add(dialog("kind", "length", "point", point.getLabelSimple()));
                }

                @Override
                public void showNumberInputDialogAngleFixed(String menu, GeoSegmentND[] pickedSegments,
                        GeoPointND[] pickedPoints, GeoElement[] selected, EuclidianController controller) {
                    segments = pickedSegments;
                    points = pickedPoints;
                    dialogs.add(dialog("kind", "angle"));
                }

                @Override
                public TextInputDialog createTextDialog(GeoText text, GeoPointND anchor, boolean world) {
                    dialogs.add(dialog("kind", "text"));
                    if (submitTextAndInputBox) {
                        textBuilder = new TextBuilder(PointerApp.this, anchor, world, false);
                    }
                    return null;
                }

                @Override
                public void showRedefineDialog(GeoElement geo, boolean allowTextDialog) {
                    if (submitTextAndInputBox) {
                        dialogs.add(dialog("kind", "redefine", "object", geo.getLabelSimple()));
                    }
                }

                @Override
                public boolean showButtonCreationDialog(int x, int y, boolean textfield) {
                    dialogs.add(dialog("kind", textfield ? "input_box" : "button", "pos", List.of(x, y)));
                    if (submitTextAndInputBox && textfield) {
                        inputBox = new ButtonDialogModel(PointerApp.this, x, y, true);
                        return true;
                    }
                    return false;
                }

                @Override
                public void showNumberInputDialogCirclePointRadius(String title, GeoPointND point,
                        EuclidianView view) {
                    dialogs.add(dialog("kind", "radius", "point", point.getLabelSimple()));
                }
            });
        }

        void enableGuiProbe() {
            if (guiProbe != null) {
                return;
            }
            var layout = new ProbeLayout();
            guiProbe = (GuiManagerInterface) Proxy.newProxyInstance(
                    GuiManagerInterface.class.getClassLoader(),
                    new Class<?>[] {GuiManagerInterface.class}, (proxy, method, args) -> {
                        switch (method.getName()) {
                            case "getLayout":
                                return layout;
                            case "getEuclidianViewCount":
                                return 1;
                            case "hasAlgebraViewShowing":
                                return algebraVisible;
                            case "showDrawingPadPopup":
                                dialogs.add(dialog("kind", "canvas_menu"));
                                break;
                            case "showPopupChooseGeo":
                                var geos = (List<?>) args[0];
                                dialogs.add(dialog("kind", "object_menu", "objects", geos.stream()
                                        .map(GeoElement.class::cast).map(GeoElement::getLabelSimple).toList()));
                                break;
                            default:
                                break;
                        }
                        if (method.getReturnType() == boolean.class) {
                            return false;
                        }
                        return null;
                    });
        }

        @Override
        public GuiManagerInterface getGuiManager() {
            return guiProbe != null ? guiProbe : super.getGuiManager();
        }

        @Override
        public boolean isUsingFullGui() {
            return guiProbe != null || super.isUsingFullGui();
        }

        @Override
        public void showRelation(GeoElement a, GeoElement b, GeoElement c, GeoElement d) {
            dialogs.add(dialog("kind", "relation", "objects",
                    List.of(a.getLabelSimple(), b.getLabelSimple())));
        }

        boolean radiusDialogRequested() {
            return !dialogs.isEmpty() && "radius".equals(((Map<?, ?>) dialogs.get(dialogs.size() - 1)).get("kind"));
        }

        void submit(String answer) {
            if (textBuilder != null) {
                textBuilder.createText(Json.write(answer), TestErrorHandler.INSTANCE, ok -> {
                    if (!ok) {
                        throw new IllegalStateException("Text dialog submission failed");
                    }
                });
                textBuilder = null;
            } else if (inputBox != null) {
                inputBox.apply(answer, "");
                inputBox = null;
            } else if (start != null) {
                var length = getKernel().getAlgebraProcessor().evaluateToNumeric(answer, true);
                DialogManager.doSegmentFixed(getKernel(), start, length);
                start = null;
            } else if (points != null) {
                // The desktop fixture gives bare angles in degrees.
                String expression = answer.matches("[+-]?[0-9]+(\\.[0-9]+)?") ? answer + "°" : answer;
                var angle = getKernel().getAlgebraProcessor().evaluateToNumeric(expression, true);
                DialogManager.doAngleFixed(getKernel(), segments, points, angle, false,
                        getActiveEuclidianView().getEuclidianController());
                points = null;
            } else if (requireSubmission) {
                throw new IllegalStateException("No dialog is waiting for Enter");
            }
        }
    }
}
