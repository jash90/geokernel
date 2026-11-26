package geokernel.harness;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.geogebra.common.awt.GColor;
import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.jre.io.MyXMLioJre;
import org.geogebra.common.kernel.CircularDefinitionException;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.LinearEquationRepresentable;
import org.geogebra.common.kernel.QuadraticEquationRepresentable;
import org.geogebra.common.kernel.arithmetic.NumberValue;
import org.geogebra.common.kernel.geos.AngleProperties;
import org.geogebra.common.kernel.geos.ChartStyleGeo;
import org.geogebra.common.kernel.kernelND.GeoPointND;
import org.geogebra.common.kernel.geos.GeoAngle;
import org.geogebra.common.kernel.geos.GeoBoolean;
import org.geogebra.common.kernel.geos.GeoButton;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoConicPart;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoFunction;
import org.geogebra.common.kernel.geos.GeoFunctionNVar;
import org.geogebra.common.kernel.geos.GeoInputBox;
import org.geogebra.common.kernel.geos.GeoImage;
import org.geogebra.common.kernel.geos.GeoLocusStroke;
import org.geogebra.common.kernel.geos.GeoLine;
import org.geogebra.common.kernel.geos.GeoLocus;
import org.geogebra.common.kernel.geos.GeoList;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoRay;
import org.geogebra.common.kernel.geos.GeoPolygon;
import org.geogebra.common.kernel.geos.GeoSegment;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.kernel.geos.GeoVector;
import org.geogebra.common.kernel.geos.HasSymbolicMode;
import org.geogebra.common.kernel.geos.TextProperties;
import org.geogebra.common.kernel.geos.HasSegmentStyle;
import org.geogebra.common.kernel.geos.PointProperties;
import org.geogebra.common.kernel.geos.SegmentStyle;
import org.geogebra.common.kernel.geos.Traceable;
import org.geogebra.common.kernel.geos.VectorHeadStyle;
import org.geogebra.common.kernel.geos.properties.FillType;
import org.geogebra.common.kernel.geos.properties.HorizontalAlignment;
import org.geogebra.common.kernel.kernelND.GeoCurveCartesianND;
import org.geogebra.common.kernel.kernelND.GeoElementND;
import org.geogebra.common.kernel.kernelND.SurfaceEvaluable.LevelOfDetail;
import org.geogebra.common.main.App;
import org.geogebra.common.main.error.ErrorHandler;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.common.plugin.EventType;
import org.geogebra.common.plugin.ScriptType;
import org.geogebra.common.kernel.statistics.GeoPieChart;
import org.geogebra.common.util.AsyncOperation;

/**
 * Saves constructions after edits, together with what
 * GeoGebra computes at that moment, to prove how a saved file relates to the
 * state it was saved from.
 *
 * <p>Usage: {@code SavedExporter <saved.json> <out dir>}. Every case has an
 * id, optionally an {@code angle_unit} ({@code degree} or {@code radiant}), raw
 * GeoGebra input lines, optional {@code hidden} labels, and {@code move} or
 * {@code set_number} steps. Steps can also enable point animation, set an
 * angle or equation style, position or style a text, rename an object, or set
 * presentation properties ({@code style}) through the setters of the element. The lines are given to the
 * algebra processor as typed (so a command that makes several objects, such as
 * {@code P = Intersect(f, k)}, gives all of them), the moves are applied, and
 * then, with nothing else done in between, the case is written as
 * {@code <out dir>/<id>.xml} (the text {@code app.getXML()} returns, or the
 * GeoGebra archive {@code <id>.ggb} when {@code archive} is true) and
 * {@code <out dir>/<id>.snapshot}: the value of every labelled object at the
 * same moment, in the field format of the golden files. The snapshot is not a
 * golden file (it has the extension {@code .snapshot}) so the golden loader
 * does not read it.
 *
 * <p>Files of the out directory that a full run did not produce are removed
 * and listed on the standard output.
 */
public final class SavedExporter {

    private SavedExporter() {
    }

    public static void main(String[] args) throws IOException {
        if (args.length < 2) {
            System.err.println("usage: SavedExporter <saved.json> <out dir> [topic ...]");
            System.exit(2);
        }
        Object parsed = Json.parse(Files.readString(Path.of(args[0]), StandardCharsets.UTF_8));
        List<?> cases = (List<?>) ((Map<?, ?>) parsed).get("cases");
        Path outDir = Path.of(args[1]);
        Files.createDirectories(outDir);
        Set<String> produced = new HashSet<>();
        List<String> topics = Arrays.asList(args).subList(2, args.length);
        for (Object o : cases) {
            Map<?, ?> input = (Map<?, ?>) o;
            String id = (String) input.get("id");
            if (!topics.isEmpty() && topics.stream().noneMatch(t -> id.startsWith("saved-" + t + "-"))) {
                continue;
            }
            try {
                export(input, outDir);
            } catch (RuntimeException e) {
                throw new IllegalStateException("case " + id + " failed: " + e.getMessage(), e);
            }
            produced.add(id + (Boolean.TRUE.equals(input.get("archive")) ? ".ggb" : ".xml"));
            produced.add(id + ".snapshot");
        }
        try (var files = Files.list(outDir)) {
            for (Path file : (Iterable<Path>) files::iterator) {
                if (topics.isEmpty() && !produced.contains(file.getFileName().toString())) {
                    System.out.println("pruned " + file.getFileName());
                    Files.delete(file);
                }
            }
        }
        System.out.printf("%d saved cases written to %s%n", cases.size(), outDir.getFileName());
    }

    private static void export(Map<?, ?> input, Path outDir) throws IOException {
        String id = (String) input.get("id");
        LocalizationCommon loc = new LocalizationCommon(2);
        loc.setResourceBundleControl(new Utf8Control());
        AppCommon app = new AppCommon(loc, new AwtFactoryCommon(), new AppConfigDefault());
        Kernel kernel = app.getKernel();
        Object unit = input.get("angle_unit");
        if ("degree".equals(unit)) {
            kernel.setAngleUnit(Kernel.ANGLE_DEGREE);
        } else if ("radiant".equals(unit)) {
            kernel.setAngleUnit(Kernel.ANGLE_RADIANT);
        } else if (unit != null) {
            throw new IllegalArgumentException("unknown angle unit " + unit);
        }
        List<GeoElement> created = new ArrayList<>();
        Map<String, byte[]> imageAssets = new LinkedHashMap<>();
        List<String> script = new ArrayList<>();
        runCommands(kernel, (List<?>) input.get("script"), script, created);
        if (input.get("images") instanceof List<?> images) {
            for (Object image : images) {
                created.add(SavedImages.create(kernel, (Map<?, ?>) image, imageAssets));
            }
        }
        if (input.get("after_images") instanceof List<?> commands) {
            runCommands(kernel, commands, script, created);
        }
        if (input.get("hidden") instanceof List<?> hidden) {
            for (Object label : hidden) {
                kernel.lookupLabel((String) label).setEuclidianVisible(false);
            }
        }
        boolean reloaded = false;
        for (Object s : (List<?>) input.get("steps")) {
            Map<?, ?> step = (Map<?, ?>) s;
            Object action = step.get("action");
            if ("load_xml".equals(action)) {
                try {
                    app.getXMLio().processXMLString((String) step.get("xml"), true, false);
                } catch (org.geogebra.common.io.XMLParseException error) {
                    throw new IllegalArgumentException(error);
                }
                reloaded = true;
                continue;
            }
            GeoElement geo = kernel.lookupLabel((String) step.get("label"));
            if (geo == null) {
                throw new IllegalArgumentException(step.get("label") + " does not exist");
            }
            if ("delete_and_undo".equals(action)) {
                app.setUndoActive(true);
                kernel.initUndoInfo();
                geo.remove();
                kernel.storeUndoInfo();
                kernel.undo();
                reloaded = true;
                continue;
            }
            if ("delete_and_run".equals(action)) {
                geo.remove();
                runCommands(kernel, List.of(step.get("input")), script, created);
                continue;
            } else if ("widget_icon".equals(action) && geo instanceof GeoButton button) {
                String name = (String) step.get("file");
                SavedImages.setIcon(button, name, imageAssets);
            } else if ("bar_image".equals(action) && geo instanceof ChartStyleGeo chart) {
                String name = (String) step.get("file");
                chart.getStyle().setBarFillType(FillType.IMAGE, 2);
                chart.getStyle().setBarImage(name, 2);
                SavedImages.addAsset(name, Boolean.TRUE.equals(step.get("large")), imageAssets);
            } else if ("move".equals(action) && geo instanceof GeoPoint p
                    && (p.isIndependent() || p.isPointOnPath())) {
                p.setCoords(number(step.get("x")), number(step.get("y")), 1.0);
            } else if ("point_coords".equals(action) && geo instanceof GeoPoint p
                    && p.isIndependent()) {
                p.setCoords(number(step.get("x")), number(step.get("y")), number(step.get("z")));
            } else if ("curve_parameter".equals(action) && geo instanceof GeoPoint p
                    && p.getPath() instanceof GeoCurveCartesianND curve) {
                p.getPathParameter().setT(number(step.get("value")));
                curve.pathChanged(p);
                p.updateCoords();
            } else if ("set_number".equals(action) && geo instanceof GeoNumeric n
                    && n.isIndependent()) {
                double value = number(step.get("value"));
                n.setValue(value);
                if (n.getDouble() != value) {
                    throw new IllegalArgumentException(step.get("label") + " clamped " + value);
                }
            } else if ("animate".equals(action) && geo instanceof GeoPoint p && p.isPointOnPath()) {
                p.setAnimating(true);
                if (!p.isAnimating()) {
                    throw new IllegalArgumentException(step.get("label") + " is not animating");
                }
            } else if ("angle_style".equals(action) && geo instanceof GeoAngle angle) {
                angle.setAngleStyle(((Number) step.get("value")).intValue());
            } else if ("line_style".equals(action) && geo instanceof GeoLine line) {
                var form = LinearEquationRepresentable.Form.valueOf((String) step.get("form"));
                if (form == LinearEquationRepresentable.Form.PARAMETRIC) {
                    line.setToParametricForm((String) step.get("parameter"));
                } else {
                    line.setEquationForm(form);
                }
            } else if ("conic_style".equals(action) && geo instanceof GeoConic conic) {
                conic.setEquationForm(
                        QuadraticEquationRepresentable.Form.valueOf((String) step.get("form")));
            } else if ("rename".equals(action)) {
                geo.rename((String) step.get("name"));
            } else if ("slider_bounds".equals(action) && geo instanceof GeoNumeric n) {
                var algebra = kernel.getAlgebraProcessor();
                NumberValue min = step.get("min") instanceof String expr
                        ? algebra.evaluateToNumeric(expr, null) : null;
                NumberValue max = step.get("max") instanceof String expr
                        ? algebra.evaluateToNumeric(expr, null) : null;
                if (min == null) {
                    n.setIntervalMin(null);
                    n.setIntervalMax(max);
                } else {
                    n.setIntervalMax(max);
                    n.setIntervalMin(min);
                }
            } else if ("object_script".equals(action)) {
                var language = ScriptType.valueOf((String) step.get("language"));
                var event = EventType.valueOf((String) step.get("event"));
                geo.setScript(language.newScript(app, (String) step.get("text")), event);
                continue;
            } else if ("click_script".equals(action)) {
                geo.setScript(ScriptType.GGBSCRIPT.newScript(app, (String) step.get("text")),
                        EventType.CLICK);
            } else if ("set_boolean".equals(action) && geo instanceof GeoBoolean b) {
                b.setValue((Boolean) step.get("value"));
            } else if ("condition".equals(action)) {
                String condition = (String) step.get("expression");
                GeoBoolean bool = kernel.getAlgebraProcessor().evaluateToBoolean(condition, null);
                try {
                    geo.setShowObjectCondition(bool);
                } catch (CircularDefinitionException e) {
                    throw new IllegalArgumentException("circular visibility condition", e);
                }
            } else if ("style".equals(action)) {
                applyStyle(kernel, geo, step);
            } else if (geo instanceof GeoText text && action instanceof String name
                    && name.startsWith("text_")) {
                applyTextStep(kernel, text, name, step);
            } else {
                throw new IllegalArgumentException("invalid saved step " + step);
            }
            geo.updateCascade();
        }
        // The state to save, and what GeoGebra computes in it.
        Map<String, Object> objects = new LinkedHashMap<>();
        boolean typedSnapshot = Boolean.TRUE.equals(input.get("typed_snapshot"));
        Iterable<GeoElement> saved = reloaded
                ? kernel.getConstruction().getGeoSetConstructionOrder() : created;
        for (GeoElement geo : saved) {
            if (geo.isLabelSet() && kernel.lookupLabel(geo.getLabelSimple()) == geo) {
                objects.put(geo.getLabelSimple(), describe(geo, typedSnapshot));
            }
        }
        if (Boolean.TRUE.equals(input.get("archive"))) {
            ByteArrayOutputStream zip = new ByteArrayOutputStream();
            ((MyXMLioJre) app.getXMLio()).writeGeoGebraFile(zip, false);
            Path target = outDir.resolve(id + ".ggb");
            byte[] bytes = SavedImages.embed(zip.toByteArray(), imageAssets);
            if (!Files.exists(target) || !GgbExporter.sameEntries(Files.readAllBytes(target), bytes)) {
                Files.write(target, bytes);
            }
        } else {
            Files.writeString(outDir.resolve(id + ".xml"), app.getXML(), StandardCharsets.UTF_8);
        }
        Map<String, Object> snapshot = new LinkedHashMap<>();
        snapshot.put("schema", 2L);
        snapshot.put("id", id);
        snapshot.put("script", script);
        snapshot.put("objects", objects);
        Files.writeString(outDir.resolve(id + ".snapshot"), Json.write(snapshot),
                StandardCharsets.UTF_8);
    }

    private static void runCommands(Kernel kernel, List<?> commands, List<String> script,
            List<GeoElement> created) {
        for (Object command : commands) {
            String line = (String) command;
            script.add(line);
            GeoElementND[] out = kernel.getAlgebraProcessor()
                    .processAlgebraCommandNoExceptionHandling(line, false,
                            new ThrowingErrorHandler(line), false, null);
            if (out == null) {
                throw new IllegalStateException("no object from " + line);
            }
            for (GeoElementND nd : out) {
                created.add(nd.toGeoElement());
            }
        }
    }

    /** Sets every listed presentation property through GeoGebra's own setters. */
    private static void applyStyle(Kernel kernel, GeoElement geo, Map<?, ?> step) {
        var algebra = kernel.getAlgebraProcessor();
        for (Map.Entry<?, ?> entry : step.entrySet()) {
            String key = (String) entry.getKey();
            Object value = entry.getValue();
            switch (key) {
                case "action", "label" -> {
                }
                case "color" -> geo.setObjColor(color((List<?>) value));
                case "alpha" -> geo.setAlphaValue(number(value));
                case "line" -> {
                    geo.setLineType(integer(((List<?>) value).get(0)));
                    geo.setLineThickness(integer(((List<?>) value).get(1)));
                }
                case "line_opacity" -> geo.setLineOpacity(integer(value));
                case "point" -> {
                    PointProperties point = (PointProperties) geo;
                    point.setPointSize(integer(((List<?>) value).get(0)));
                    point.setPointStyle(integer(((List<?>) value).get(1)));
                }
                case "fill" -> {
                    List<?> fill = (List<?>) value;
                    geo.setFillType(FillType.valueOf((String) fill.get(0)));
                    geo.setHatchingAngle(integer(fill.get(1)));
                    geo.setHatchingDistance(integer(fill.get(2)));
                }
                case "fill_symbol" -> geo.setFillSymbol((String) value);
                case "inverse_fill" -> geo.setInverseFill((Boolean) value);
                case "layer" -> geo.setLayer(integer(value));
                case "fixed" -> {
                    geo.setFixed((Boolean) value);
                    if (geo.isLocked() != (Boolean) value) {
                        throw new IllegalArgumentException(geo.getLabelSimple() + " cannot be fixed");
                    }
                }
                case "auxiliary" -> geo.setAuxiliaryObject((Boolean) value);
                case "selection_allowed" -> geo.setSelectionAllowed((Boolean) value);
                case "dynamic_color" -> geo.setColorFunction(
                        algebra.evaluateToList("{" + String.join(", ", strings(value)) + "}"));
                case "color_space" -> geo.setColorSpace(integer(value));
                case "label_mode" -> geo.setLabelMode(integer(value));
                case "label_visible" -> geo.setLabelVisible((Boolean) value);
                case "label_offset" -> {
                    geo.labelOffsetX = integer(((List<?>) value).get(0));
                    geo.labelOffsetY = integer(((List<?>) value).get(1));
                }
                case "caption" -> geo.setCaption((String) value);
                case "input_length" -> ((GeoInputBox) geo).setLength(integer(value));
                case "font" -> {
                    List<?> font = (List<?>) value;
                    TextProperties text = (TextProperties) geo;
                    text.setSerifFont((Boolean) font.get(0));
                    text.setFontSizeMultiplier(number(font.get(1)));
                    text.setFontStyle(integer(font.get(2)));
                }
                case "symbolic" -> ((HasSymbolicMode) geo).setSymbolicMode((Boolean) value, false);
                case "content_serif" -> ((GeoInputBox) geo).setSerifContent((Boolean) value);
                case "text_align" -> ((GeoInputBox) geo)
                        .setAlignment(HorizontalAlignment.valueOf((String) value));
                case "temp_input" -> {
                    List<?> input = (List<?>) value;
                    ((GeoInputBox) geo).setTempUserInput((String) input.get(0), (String) input.get(1));
                }
                case "dropdown" -> {
                    GeoList list = (GeoList) geo;
                    list.setDrawAsComboBox(true);
                    list.setSelectedIndex(integer(value));
                }
                case "list_type" -> ((GeoList) geo).setTypeStringForXML((String) value);
                case "interpolate" -> ((GeoImage) geo).setInterpolate((Boolean) value);
                case "surface_quality" -> ((GeoFunctionNVar) geo).setLevelOfDetail(LevelOfDetail.QUALITY);
                case "split_parent" -> ((GeoLocusStroke) geo).setSplitParentLabel((String) value);
                case "spreadsheet_trace" -> {
                    geo.setSpreadsheetTrace(true);
                    var trace = geo.getTraceSettings();
                    trace.traceColumn1 = 2;
                    trace.traceColumn2 = 2;
                    trace.traceRow1 = 1;
                    trace.traceRow2 = 5;
                    trace.tracingRow = 2;
                    trace.numRows = 4;
                    trace.headerOffset = 1;
                    trace.doRowLimit = true;
                    trace.pause = true;
                }
                case "point_polar" -> ((GeoPoint) geo).setMode(Kernel.COORD_POLAR);
                case "vector_polar" -> ((GeoVector) geo).setMode(Kernel.COORD_POLAR);
                case "vertical_increment" ->
                        ((GeoPoint) geo).setVerticalIncrement(algebra.evaluateToNumeric((String) value, null));
                case "vector_start" -> {
                    try {
                        var point = (GeoPointND) kernel.lookupLabel((String) value);
                        if (point == null) {
                            point = algebra.evaluateToPoint((String) value, null, true);
                        }
                        ((GeoVector) geo).setStartPoint(point);
                    } catch (CircularDefinitionException ex) {
                        throw new IllegalArgumentException("circular vector start", ex);
                    }
                }
                case "slope_triangle_size" -> ((GeoNumeric) geo).setSlopeTriangleSize(integer(value));
                case "show_on_axis" -> ((GeoFunction) geo).setShowOnAxis((Boolean) value);
                case "simplify_coefficients" -> ((GeoFunction) geo).setSimplifyCoefficients((Boolean) value);
                case "table_column" -> {
                    GeoFunction function = (GeoFunction) geo;
                    function.setTableColumn(integer(value));
                    function.setPointsVisible(false);
                }
                case "outlying_intersections" -> ((GeoSegment) geo).setAllowOutlyingIntersections((Boolean) value);
                case "keep_type" -> ((GeoSegment) geo).setKeepTypeOnGeometricTransform((Boolean) value);
                case "mask" -> ((GeoPolygon) geo).setIsMask((Boolean) value);
                case "slice_color", "bar_color" ->
                        ((ChartStyleGeo) geo).getStyle().setBarColor(color((List<?>) value), 2);
                case "bar_alpha" -> ((ChartStyleGeo) geo).getStyle().setBarAlpha(
                        Float.parseFloat((String) value), 2);
                case "bar_fill_type" ->
                        ((ChartStyleGeo) geo).getStyle().setBarFillType(
                                FillType.valueOf((String) value), 2);
                case "angle_style" -> ((AngleProperties) geo).setAngleStyle(integer(value));
                case "emphasize_right_angle" ->
                        ((AngleProperties) geo).setEmphasizeRightAngle((Boolean) value);
                case "button_size" -> {
                    GeoButton button = (GeoButton) geo;
                    List<?> dimensions = (List<?>) value;
                    button.setWidth(number(dimensions.get(0)));
                    button.setHeight(number(dimensions.get(1)));
                    button.setFixedSize(true);
                }
                case "button_position" -> {
                    GeoButton button = (GeoButton) geo;
                    List<?> position = (List<?>) value;
                    button.setAbsoluteScreenLoc(integer(position.get(0)), integer(position.get(1)));
                    button.setAbsoluteScreenLocActive(true);
                }
                case "tooltip_mode" -> geo.setTooltipMode(integer(value));
                case "trace" -> ((Traceable) geo).setTrace((Boolean) value);
                case "background" -> {
                    List<?> rgba = (List<?>) value;
                    geo.setBackgroundColor(GColor.newColor(integer(rgba.get(0)),
                            integer(rgba.get(1)), integer(rgba.get(2)), integer(rgba.get(3))));
                }
                case "decoration" -> geo.setDecorationType(integer(value));
                case "line_hidden" -> geo.setLineTypeHidden(integer(value));
                case "views" -> applyViews(geo, integer(value));
                case "ordering" -> geo.setOrdering(number(value));
                case "dynamic_caption" ->
                        geo.setDynamicCaption((GeoText) kernel.lookupLabel((String) value));
                case "breakpoint" -> geo.setConsProtocolBreakpoint((Boolean) value);
                case "algebra_label" -> geo.setAlgebraLabelVisible((Boolean) value);
                case "fixed_size" -> geo.setFixedSize((Boolean) value);
                case "show_trimmed" -> geo.setShowTrimmedIntersectionLines((Boolean) value);
                case "knob_size" -> ((GeoNumeric) geo).setSliderBlobSize(number(value));
                case "ends" -> {
                    List<?> ends = (List<?>) value;
                    HasSegmentStyle styled = (HasSegmentStyle) geo;
                    styled.setStartStyle(SegmentStyle.valueOf((String) ends.get(0)));
                    styled.setEndStyle(SegmentStyle.valueOf((String) ends.get(1)));
                }
                case "head_style" ->
                        ((GeoVector) geo).setHeadStyle(VectorHeadStyle.values()[integer(value)]);
                case "draw_arrows" -> ((GeoLocus) geo).drawAsArrows((Boolean) value);
                case "decimals" -> {
                    if (geo instanceof GeoInputBox box) {
                        box.setPrintDecimals(integer(value), true);
                    } else if (geo instanceof GeoList list) {
                        list.setPrintDecimals(integer(value), true);
                    } else {
                        ((GeoText) geo).setPrintDecimals(integer(value), true);
                    }
                }
                case "figures" -> {
                    if (geo instanceof GeoInputBox box) {
                        box.setPrintFigures(integer(value), true);
                    } else {
                        ((GeoText) geo).setPrintFigures(integer(value), true);
                    }
                }
                case "arc_size" -> ((GeoAngle) geo).setArcSize(integer(value));
                case "checkbox_fixed" -> ((GeoBoolean) geo).setCheckboxFixed((Boolean) value);
                case "slider" -> applySlider((GeoNumeric) geo, (Map<?, ?>) value);
                case "animation" -> {
                    Map<?, ?> animation = (Map<?, ?>) value;
                    if (animation.get("step") instanceof String step2) {
                        geo.setAnimationStep(algebra.evaluateToNumeric(step2, null));
                        if (geo instanceof GeoNumeric n) {
                            n.setAutoStep(false);
                        }
                    }
                    if (animation.get("speed") instanceof String speed) {
                        geo.setAnimationSpeedObject(algebra.evaluateToNumeric(speed, null));
                    }
                    if (animation.get("mode") != null) {
                        geo.setAnimationType(integer(animation.get("mode")));
                    }
                    if (Boolean.TRUE.equals(animation.get("playing"))) {
                        geo.setAnimating(true);
                        if (!geo.isAnimating()) {
                            throw new IllegalArgumentException(
                                    geo.getLabelSimple() + " is not animating");
                        }
                    }
                }
                default -> throw new IllegalArgumentException("unknown style property " + key);
            }
            // The headless view keeps drawables; refresh them before the next setter repaints.
            geo.updateRepaint();
        }
    }

    /** The view bits of a saved {@code <show ev=>}, applied as GeoGebra's reader applies them. */
    private static void applyViews(GeoElement geo, int bits) {
        if ((bits & 1) == 0) {
            geo.addView(App.VIEW_EUCLIDIAN);
        } else {
            geo.removeView(App.VIEW_EUCLIDIAN);
        }
        if ((bits & 2) == 2) {
            geo.addView(App.VIEW_EUCLIDIAN2);
        } else {
            geo.removeView(App.VIEW_EUCLIDIAN2);
        }
        if ((bits & 4) == 4) {
            geo.addViews3D();
        }
        if ((bits & 8) == 8) {
            geo.removeViews3D();
        }
        if ((bits & 16) == 16) {
            geo.setVisibleInViewForPlane(true);
        }
        if ((bits & 32) == 32) {
            geo.setVisibleInViewForPlane(false);
        }
    }

    private static void applySlider(GeoNumeric slider, Map<?, ?> value) {
        for (Map.Entry<?, ?> entry : value.entrySet()) {
            Object v = entry.getValue();
            switch ((String) entry.getKey()) {
                case "absolute" -> slider.setAbsoluteScreenLocActive((Boolean) v);
                case "position" -> slider.setSliderLocation(number(((List<?>) v).get(0)),
                        number(((List<?>) v).get(1)), true);
                case "width" -> slider.setSliderWidth(number(v), true);
                case "horizontal" -> slider.setSliderHorizontal((Boolean) v);
                case "fixed" -> slider.setSliderFixed((Boolean) v);
                case "show_algebra" -> slider.setAVSliderOrCheckboxVisible((Boolean) v);
                case "blob_size" -> slider.setSliderBlobSize(number(v));
                default -> throw new IllegalArgumentException("unknown slider property " + entry.getKey());
            }
        }
    }

    private static GColor color(List<?> rgb) {
        return GColor.newColor(integer(rgb.get(0)), integer(rgb.get(1)), integer(rgb.get(2)));
    }

    private static int integer(Object value) {
        return Math.toIntExact((Long) value);
    }

    private static List<String> strings(Object value) {
        List<String> out = new ArrayList<>();
        for (Object item : (List<?>) value) {
            out.add((String) item);
        }
        return out;
    }

    private static void applyTextStep(Kernel kernel, GeoText text, String action, Map<?, ?> step) {
        switch (action) {
            case "text_screen":
                text.setAbsoluteScreenLocActive(true);
                text.setAbsoluteScreenLoc(((Number) step.get("x")).intValue(),
                        ((Number) step.get("y")).intValue());
                break;
            case "text_world":
                text.setRealWorldLoc(number(step.get("x")), number(step.get("y")));
                break;
            case "text_anchor":
                try {
                    text.setStartPoint((GeoPoint) kernel.lookupLabel((String) step.get("point")));
                } catch (CircularDefinitionException e) {
                    throw new IllegalArgumentException("circular text anchor " + step, e);
                }
                break;
            case "text_font":
                text.setLaTeX(Boolean.TRUE.equals(step.get("latex")), false);
                text.setSerifFont(Boolean.TRUE.equals(step.get("serif")));
                text.setFontSizeMultiplier(number(step.get("size")));
                text.setFontStyle(((Number) step.get("style")).intValue());
                break;
            default:
                throw new IllegalArgumentException("invalid saved text step " + step);
        }
    }

    private static double number(Object value) {
        if (value instanceof Long) {
            return ((Long) value).doubleValue();
        }
        return (Double) value;
    }

    /** The golden-file form of an object (only the types the saved cases use). */
    private static Map<String, Object> describe(GeoElement geo, boolean typedSnapshot) {
        // Opt in to the typed forms while keeping the legacy snapshots reproducible.
        if (typedSnapshot && (geo instanceof GeoRay || geo instanceof GeoFunctionNVar)) {
            return Session.describe(geo, Session.typeOf(geo));
        }
        if (geo instanceof org.geogebra.common.kernel.geos.GeoImage image) {
            return SavedImages.describe(image);
        }
        Map<String, Object> obj = new LinkedHashMap<>();
        Map<String, Object> f = new LinkedHashMap<>();
        boolean ok = geo.isDefined();
        if (geo instanceof GeoBoolean || geo instanceof GeoFunction || geo instanceof GeoVector
                || geo instanceof GeoList || geo instanceof GeoPolygon || geo instanceof GeoText
                || geo.isGeoLocus() || geo.isGeoPolyLine() || geo.isGeoCurveCartesian()
                || geo instanceof GeoConicPart
                || geo instanceof GeoPieChart
                || geo.isGeoImplicitCurve()) {
            return Session.describe(geo, Session.typeOf(geo));
        } else if (geo instanceof GeoFunctionNVar) {
            return Session.describe(geo, "function");
        } else if (geo instanceof GeoButton || geo instanceof GeoInputBox) {
            obj.put("type", geo.getTypeString());
        } else if (geo instanceof GeoPoint) {
            GeoPoint p = (GeoPoint) geo;
            obj.put("type", "point");
            ok &= !p.isInfinite();
            if (ok) {
                f.put("x", p.getInhomX());
                f.put("y", p.getInhomY());
            }
        } else if (geo instanceof GeoSegment) {
            GeoSegment s = (GeoSegment) geo;
            obj.put("type", "segment");
            ok &= !s.getStartPoint().isInfinite() && !s.getEndPoint().isInfinite();
            if (ok) {
                f.put("x1", s.getStartPoint().getInhomX());
                f.put("y1", s.getStartPoint().getInhomY());
                f.put("x2", s.getEndPoint().getInhomX());
                f.put("y2", s.getEndPoint().getInhomY());
            }
        } else if (geo instanceof GeoLine) {
            GeoLine l = (GeoLine) geo;
            obj.put("type", "line");
            if (ok) {
                f.put("a", l.x);
                f.put("b", l.y);
                f.put("c", l.z);
            }
        } else if (geo instanceof GeoConic) {
            GeoConic c = (GeoConic) geo;
            if (c.getType() != GeoConic.CONIC_CIRCLE) {
                return Session.describe(c, "conic");
            }
            obj.put("type", "circle");
            if (ok) {
                f.put("cx", c.getMidpoint2D().getX());
                f.put("cy", c.getMidpoint2D().getY());
                f.put("r", c.getHalfAxis(0));
            }
        } else if (geo instanceof GeoNumeric) {
            obj.put("type", Session.typeOf(geo));
            if (ok) {
                f.put("value", ((GeoNumeric) geo).getDouble());
            }
        } else {
            throw new IllegalStateException("unsupported object " + geo.getClass());
        }
        ok &= f.values().stream().allMatch(v -> Double.isFinite((Double) v));
        obj.put("defined", ok);
        if (ok) {
            obj.putAll(f);
        }
        return obj;
    }

    private static final class ThrowingErrorHandler implements ErrorHandler {
        private final String line;

        ThrowingErrorHandler(String line) {
            this.line = line;
        }

        @Override
        public void showError(String msg) {
            throw new IllegalStateException("GeoGebra error in '" + line + "': " + msg);
        }

        @Override
        public void showCommandError(String command, String message) {
            throw new IllegalStateException("GeoGebra error in '" + line + "': " + message);
        }

        @Override
        public String getCurrentCommand() {
            return line;
        }

        @Override
        public boolean onUndefinedVariables(String string, AsyncOperation<String[]> callback) {
            throw new IllegalStateException("undefined variables in '" + line + "': " + string);
        }

        @Override
        public void resetError() {
            // nothing to reset
        }
    }
}
