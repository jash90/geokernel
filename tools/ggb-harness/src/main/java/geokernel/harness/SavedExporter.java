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
import org.geogebra.common.kernel.geos.GeoAngle;
import org.geogebra.common.kernel.geos.GeoBoolean;
import org.geogebra.common.kernel.geos.GeoButton;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoFunction;
import org.geogebra.common.kernel.geos.GeoFunctionNVar;
import org.geogebra.common.kernel.geos.GeoInputBox;
import org.geogebra.common.kernel.geos.GeoLine;
import org.geogebra.common.kernel.geos.GeoList;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoPolygon;
import org.geogebra.common.kernel.geos.GeoSegment;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.kernel.geos.GeoVector;
import org.geogebra.common.kernel.kernelND.GeoCurveCartesianND;
import org.geogebra.common.kernel.kernelND.GeoElementND;
import org.geogebra.common.main.error.ErrorHandler;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.common.plugin.EventType;
import org.geogebra.common.plugin.ScriptType;
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
 * angle or equation style, position or style a text, or rename an object. The lines are given to the
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
        for (Object s : (List<?>) input.get("steps")) {
            Map<?, ?> step = (Map<?, ?>) s;
            GeoElement geo = kernel.lookupLabel((String) step.get("label"));
            if (geo == null) {
                throw new IllegalArgumentException(step.get("label") + " does not exist");
            }
            Object action = step.get("action");
            if ("move".equals(action) && geo instanceof GeoPoint p
                    && (p.isIndependent() || p.isPointOnPath())) {
                p.setCoords(number(step.get("x")), number(step.get("y")), 1.0);
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
        for (GeoElement geo : created) {
            if (geo.isLabelSet() && kernel.lookupLabel(geo.getLabelSimple()) == geo) {
                objects.put(geo.getLabelSimple(), describe(geo));
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
    private static Map<String, Object> describe(GeoElement geo) {
        if (geo instanceof org.geogebra.common.kernel.geos.GeoImage image) {
            return SavedImages.describe(image);
        }
        Map<String, Object> obj = new LinkedHashMap<>();
        Map<String, Object> f = new LinkedHashMap<>();
        boolean ok = geo.isDefined();
        if (geo instanceof GeoBoolean || geo instanceof GeoFunction || geo instanceof GeoVector
                || geo instanceof GeoList || geo instanceof GeoPolygon || geo instanceof GeoText
                || geo.isGeoLocus() || geo.isGeoPolyLine() || geo.isGeoCurveCartesian()
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
