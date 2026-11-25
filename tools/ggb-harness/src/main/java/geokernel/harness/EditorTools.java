package geokernel.harness;

import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.imageio.ImageIO;

import org.geogebra.common.awt.GColor;
import org.geogebra.common.awt.GPoint;
import org.geogebra.common.euclidian.event.PointerEventType;
import org.geogebra.common.euclidian.EuclidianConstants;
import org.geogebra.common.euclidian.EuclidianView;
import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.DialogManagerNoGui;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.MyImageCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.kernel.ModeSetter;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoFunction;
import org.geogebra.common.kernel.geos.GeoImage;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.main.DialogManager;
import org.geogebra.common.main.GuiManagerInterface;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.common.plugin.EuclidianStyleConstants;
import org.geogebra.common.util.ImageManagerCommon;
import org.geogebra.test.TestEvent;

/** Records Classic pointer tools with headless replacements for desktop dialogs. */
public final class EditorTools {
    private EditorTools() {
    }

    private static final class Probe extends AppCommon {
        private final Map<String, Object> inspector = new LinkedHashMap<>();
        private GeoImage raster;
        private GuiManagerInterface fixtureGui;

        Probe() {
            super(localization(), new AwtFactoryCommon(), new AppConfigDefault());
            setAppletFlag(false);
        }

        @Override
        public DialogManager getDialogManager() {
            return new DialogManagerNoGui(this, new String[0]) {
                @Override
                public boolean showFunctionInspector(GeoFunction function) {
                    var view = getActiveEuclidianView();
                    var mouse = view.getEuclidianController().getMouseLoc();
                    double center = view.toRealWorldCoordX(mouse.getX());
                    double step = 0.25 * view.getGridDistances()[0];
                    double x = center - 4 * step;
                    var rows = new ArrayList<List<Double>>();
                    for (int i = 0; i < 9; i++, x += step) {
                        rows.add(List.of(x, function.value(x)));
                    }
                    inspector.put("label", function.getLabelSimple());
                    inspector.put("rows", rows);
                    return true;
                }
            };
        }

        @Override
        public GuiManagerInterface getGuiManager() {
            return fixtureGui;
        }

        void allowFixtureImage(Path imageFile) throws java.io.IOException {
            var fixture = ImageIO.read(imageFile.toFile());
            int width = fixture.getWidth();
            int height = fixture.getHeight();
            var images = new ImageManagerCommon();
            images.addExternalImage(new MyImageCommon(width, height), "fixture.png");
            setImageManager(images);
            fixtureGui = (GuiManagerInterface) Proxy.newProxyInstance(
                    GuiManagerInterface.class.getClassLoader(),
                    new Class<?>[] { GuiManagerInterface.class }, (proxy, method, args) -> {
                        if (method.getName().equals("loadImage")) {
                            if (args[0] instanceof GeoPoint anchor) {
                                getSelectionManager().addSelectedGeo(anchor);
                            }
                            raster = new GeoImage(getKernel().getConstruction());
                            raster.setImageFileName("fixture.png", width, height);
                            getImageManager().setCornersFromSelection(raster, this);
                        }
                        if (method.getReturnType() == boolean.class) {
                            return false;
                        }
                        return null;
                    });
        }
    }

    private static LocalizationCommon localization() {
        var loc = new LocalizationCommon(2);
        loc.setResourceBundleControl(new Utf8Control());
        return loc;
    }

    public static void main(String[] args) throws Exception {
        var inputs = (Map<?, ?>) Json.parse(Files.readString(Path.of(args[0])));
        var cases = new ArrayList<Map<String, Object>>();
        var image = Path.of(args[0]).resolveSibling("fixture.png");
        for (Object entry : (List<?>) inputs.get("cases")) {
            cases.add(record((Map<?, ?>) entry, image));
        }
        Files.writeString(Path.of(args[1]), Json.write(Map.of("scenarios", cases)));
    }

    private static Map<String, Object> record(Map<?, ?> spec, Path fixture) throws Exception {
        var app = new Probe();
        var kernel = app.getKernel();
        EuclidianView view = app.getActiveEuclidianView();
        view.setCoordSystem(446, 373.5, 50, 50);
        view.getSettings().setPointCapturing(EuclidianStyleConstants.POINT_CAPTURING_OFF);
        var controller = view.getEuclidianController();
        controller.setDraggingDelay(0);
        var script = (List<?>) spec.get("initial_script");
        for (Object line : script) {
            if (line.toString().startsWith("$drawingStyle(")) {
                var source = kernel.lookupLabel("A");
                source.setObjColor(GColor.newColor(40, 90, 160));
                source.setLayer(2);
                source.setLabelVisible(false);
                source.setLineThickness(7);
                source.updateRepaint();
            } else {
                kernel.getAlgebraProcessor().processAlgebraCommand(line.toString(), false);
            }
        }
        var con = kernel.getConstruction();
        var original = con.getGeoSetConstructionOrder().stream().map(GeoElement::getLabelSimple).toList();
        app.setUndoActive(true);
        kernel.initUndoInfo();
        int before = app.getUndoManager().getHistorySize();
        int mode = (int) ((Long) spec.get("tool_mode")).longValue();
        var events = (List<?>) spec.get("events");
        boolean image = Boolean.TRUE.equals(spec.get("image"));
        if (image) {
            // Classic opens the chooser during toolbar activation and uses the
            // point under the pointer. Prime that hit with the placement event.
            var at = pointer(view, (Map<?, ?>) events.get(0));
            controller.wrapMouseMoved(at);
            controller.wrapMousePressed(at);
            controller.wrapMouseReleased(at);
            view.setHits(new GPoint(at.getX(), at.getY()), PointerEventType.MOUSE);
            app.allowFixtureImage(fixture);
        }
        view.setMode(mode, ModeSetter.TOOLBAR);
        if (mode == EuclidianConstants.MODE_FREEHAND_FUNCTION) {
            controller.prepareModeForFreehand();
        }
        if (!image) {
            for (Object item : events) {
                var event = (Map<?, ?>) item;
                switch ((String) event.get("type")) {
                    case "mouse_down" -> {
                        var pointer = pointer(view, event);
                        controller.wrapMouseMoved(pointer);
                        controller.wrapMousePressed(pointer);
                    }
                    case "mouse_move" -> controller.wrapMouseDragged(pointer(view, event), true);
                    case "mouse_up" -> controller.wrapMouseReleased(pointer(view, event));
                    case "select_tool" -> view.setMode(((Long) event.get("mode")).intValue(), ModeSetter.TOOLBAR);
                    default -> throw new IllegalArgumentException("unknown pointer event");
                }
            }
        }
        var expected = new LinkedHashMap<String, Object>();
        expected.put("tool_mode_after", controller.getMode());
        if (!image) {
            expected.put("undo_steps_added", app.getUndoManager().getHistorySize() - before);
        }
        if (spec.get("points") instanceof List<?> labels) {
            var points = new LinkedHashMap<String, Object>();
            for (Object label : labels) {
                var point = (GeoPoint) kernel.lookupLabel(label.toString());
                points.put(label.toString(), List.of(point.getInhomX(), point.getInhomY()));
            }
            expected.put("editor_points", points);
        }
        if (spec.get("labels") instanceof List<?> labels) {
            var styles = new LinkedHashMap<String, Object>();
            for (Object label : labels) {
                var object = kernel.lookupLabel(label.toString());
                var style = new LinkedHashMap<String, Object>();
                style.put("show_label", object.isLabelVisible());
                if (Boolean.TRUE.equals(spec.get("style"))) {
                    var rgb = object.getObjectColor();
                    style.put("color", List.of(rgb.getRed(), rgb.getGreen(), rgb.getBlue()));
                    style.put("layer", object.getLayer());
                    style.put("line_thickness", object.getLineThickness());
                }
                styles.put(label.toString(), style);
            }
            expected.put("editor_styles", styles);
        }
        if (Boolean.TRUE.equals(spec.get("function"))) {
            var f = (GeoFunction) kernel.lookupLabel("f");
            expected.put("freehand_samples", List.of(
                    List.of(1.0, f.value(1)), List.of(2.0, f.value(2)), List.of(2.5, f.value(2.5))));
        }
        if (Boolean.TRUE.equals(spec.get("inspector"))) {
            expected.put("inspector", app.inspector);
        }
        if (image) {
            if (app.raster == null) {
                throw new IllegalStateException("image chooser was not invoked");
            }
            var anchor = app.raster.getStartPoint(0);
            expected.put("image_anchor", List.of(anchor.getInhomX(), anchor.getInhomY()));
        } else {
            expected.put("created_labels", con.getGeoSetConstructionOrder().stream()
                    .map(GeoElement::getLabelSimple).filter(label -> !original.contains(label)).toList());
        }
        var output = new LinkedHashMap<String, Object>();
        output.put("name", spec.get("name"));
        output.put("source", "Classic Java editor-tools probe, GeoGebra " + System.getenv("GEOGEBRA_REV"));
        for (String key : List.of("tool_mode", "initial_script", "events")) {
            output.put(key, spec.get(key));
        }
        output.put("expected", expected);
        return output;
    }

    private static TestEvent pointer(EuclidianView view, Map<?, ?> event) {
        var xy = (List<?>) event.get("pos");
        return new TestEvent(view.toScreenCoordX(((Number) xy.get(0)).doubleValue()),
                view.toScreenCoordY(((Number) xy.get(1)).doubleValue()));
    }
}
