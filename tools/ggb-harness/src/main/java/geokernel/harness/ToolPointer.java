package geokernel.harness;

import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.geogebra.common.euclidian.EuclidianController;
import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.gui.dialog.TextInputDialog;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.DialogManagerNoGui;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.kernel.kernelND.GeoPointND;
import org.geogebra.common.kernel.kernelND.GeoSegmentND;
import org.geogebra.common.main.DialogManager;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.test.TestEvent;

/** Records the controller's construction state after each pointer gesture. */
public final class ToolPointer {
    private ToolPointer() {
    }

    public static void main(String[] args) throws Exception {
        var captures = new ArrayList<Object>();
        for (String input : List.of(args[0], args[1])) {
            var scenarios = (List<?>) ((Map<?, ?>) Json.parse(Files.readString(Path.of(input))))
                    .get("scenarios");
            for (Object entry : scenarios) {
                var scenario = (Map<?, ?>) entry;
                String name = (String) scenario.get("name");
                if (name.startsWith("tool_") && (input.equals(args[0]) || List.of(
                        "tool_input_box_places_empty_widget", "tool_create_list_from_rectangle",
                        "tool_text_places_literal", "tool_locus_uses_path_point",
                        "tool_pen_records_drag").contains(name))) {
                    captures.add(record(scenario));
                }
            }
        }
        var output = new LinkedHashMap<String, Object>();
        output.put("upstream", args[3]);
        output.put("scenarios", captures);
        Files.writeString(Path.of(args[2]), Json.write(output));
    }

    private static Map<String, Object> record(Map<?, ?> input) throws Exception {
        var app = new PointerApp();
        var view = app.getActiveEuclidianView();
        app.setAppletFlag(false);
        view.setCoordSystem(446, 373.5, 50, 50);
        var controller = view.getEuclidianController();
        controller.setDraggingDelay(0);
        app.setMode(((Number) input.get("tool_mode")).intValue());
        for (Object line : (List<?>) input.get("initial_script")) {
            if (app.getKernel().getAlgebraProcessor().processAlgebraCommand((String) line, false)
                    == null) {
                throw new IllegalArgumentException("Could not run " + line);
            }
        }
        app.getKernel().setUndoActive(true);
        app.getKernel().initUndoInfo();

        var result = new LinkedHashMap<String, Object>();
        for (String key : List.of("name", "tool_mode", "initial_script", "events")) {
            result.put(key, input.get(key));
        }
        var initial = snapshot(app);
        result.put("initial", initial);
        var frames = new ArrayList<Object>();
        boolean dragging = false;
        String answer = "";
        for (Object item : (List<?>) input.get("events")) {
            var event = (Map<?, ?>) item;
            String type = (String) event.get("type");
            var pos = (List<?>) event.get("pos");
            var pointer = pos == null ? null : new TestEvent(
                    view.toScreenCoordX(((Number) pos.get(0)).doubleValue()),
                    view.toScreenCoordY(((Number) pos.get(1)).doubleValue()));
            switch (type) {
                case "mouse_down" -> {
                    controller.wrapMouseMoved(pointer);
                    controller.wrapMousePressed(pointer);
                    dragging = true;
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
                case "key" -> {
                    if (!"Enter".equals(event.get("key"))) {
                        throw new IllegalArgumentException("Unsupported dialog key " + event);
                    }
                    app.submitNumber(answer);
                }
                default -> throw new IllegalArgumentException("Unsupported pointer event " + type);
            }
            if ("mouse_up".equals(type) || "key".equals(type)) {
                var frame = new LinkedHashMap<String, Object>();
                frame.put("t", event.get("t"));
                frame.put("objects", snapshot(app));
                frames.add(frame);
            }
        }
        result.put("frames", frames);
        result.put("dialogs", app.dialogs);
        var undo = new ArrayList<Object>();
        for (int i = 0; i < 4 && !snapshot(app).equals(initial); i++) {
            app.getKernel().undo();
            undo.add(snapshot(app));
        }
        result.put("undo", undo);
        return result;
    }

    private static Map<String, Object> snapshot(PointerApp app) {
        var result = new LinkedHashMap<String, Object>();
        for (GeoElement geo : app.getKernel().getConstruction().getGeoSetConstructionOrder()) {
            var state = new LinkedHashMap<>(Session.describe(geo, Session.typeOf(geo)));
            if (geo instanceof GeoPointND point) {
                state.put("on_path", point.isPointOnPath());
                state.put("in_region", point.isPointInRegion());
            }
            result.put(geo.getLabelSimple(), state);
        }
        return result;
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

    private static final class PointerApp extends AppCommon {
        private final List<Object> dialogs = new ArrayList<>();
        private GeoPointND start;
        private GeoPointND[] points;
        private GeoSegmentND[] segments;

        PointerApp() throws ReflectiveOperationException {
            super(localization(), new AwtFactoryCommon(), new AppConfigDefault());
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
                    return null;
                }

                @Override
                public boolean showButtonCreationDialog(int x, int y, boolean textfield) {
                    dialogs.add(dialog("kind", textfield ? "input_box" : "button", "pos", List.of(x, y)));
                    return false;
                }
            });
        }

        @Override
        public void showRelation(GeoElement a, GeoElement b, GeoElement c, GeoElement d) {
            dialogs.add(dialog("kind", "relation", "objects",
                    List.of(a.getLabelSimple(), b.getLabelSimple())));
        }

        void submitNumber(String answer) {
            if (start != null) {
                var length = new GeoNumeric(getKernel().getConstruction(), Double.parseDouble(answer));
                DialogManager.doSegmentFixed(getKernel(), start, length);
                start = null;
            } else if (points != null) {
                double radians = Math.toRadians(Double.parseDouble(answer));
                var angle = new GeoNumeric(getKernel().getConstruction(), radians);
                DialogManager.doAngleFixed(getKernel(), segments, points, angle, false,
                        getActiveEuclidianView().getEuclidianController());
                points = null;
            }
        }
    }
}
