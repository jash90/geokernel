package geokernel.harness;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.algos.AlgoElement;
import org.geogebra.common.kernel.algos.AlgoJoinPointsRay;
import org.geogebra.common.kernel.algos.AlgoRayPointVector;
import org.geogebra.common.kernel.commands.AlgebraProcessor;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoLine;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoRay;
import org.geogebra.common.kernel.geos.GeoSegment;
import org.geogebra.common.kernel.geos.GeoVector;
import org.geogebra.common.kernel.kernelND.GeoElementND;
import org.geogebra.common.kernel.matrix.Coords;
import org.geogebra.common.main.error.ErrorHandler;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.common.util.AsyncOperation;
import org.geogebra.common.util.StringUtil;

/**
 * One fresh headless GeoGebra app (Classic configuration) that runs script
 * lines, applies steps and records snapshots in the golden-data format.
 * Every surprise (an unexpected label, class or conic type) is an error.
 */
final class Session {

    private static final Pattern LINE =
            Pattern.compile("^([A-Za-z]+(?:_\\{[A-Za-z0-9]+\\})?) = (.+)$");
    private static final Pattern NUMBER = Pattern.compile("^-?\\d+(\\.\\d+)?$");
    private static final Pattern POINT =
            Pattern.compile("^\\(-?\\d+(\\.\\d+)?, -?\\d+(\\.\\d+)?\\)$");
    private static final Pattern COMMAND = Pattern.compile("^([A-Za-z]+)\\[?\\(.*\\)$");

    private final AppCommon app;
    private final Kernel kernel;
    private final AlgebraProcessor processor;
    /**
     * Scripted objects by label. GeoGebra gives an object that is undefined
     * when it is created no label until it becomes defined, so snapshots hold
     * on to the objects themselves instead of looking labels up.
     */
    private final Map<String, GeoElement> objects = new LinkedHashMap<>();

    Session() {
        LocalizationCommon loc = new LocalizationCommon(2);
        loc.setResourceBundleControl(new Utf8Control());
        app = new AppCommon(loc, new AwtFactoryCommon(), new AppConfigDefault());
        kernel = app.getKernel();
        processor = kernel.getAlgebraProcessor();
    }

    Kernel kernel() {
        return kernel;
    }

    /** Runs one `label = definition` line and checks what GeoGebra created. */
    GeoElement run(String line) {
        Matcher m = LINE.matcher(line);
        if (!m.matches()) {
            throw new IllegalArgumentException("not of the form 'label = definition': " + line);
        }
        String label = m.group(1);
        String definition = m.group(2).trim();
        String transformed = transformedType(definition);
        String expected = transformed != null ? transformed : expectedType(label, definition);
        if (objects.containsKey(label) || kernel.lookupLabel(label) != null) {
            throw new IllegalArgumentException("label used twice: " + label);
        }
        GeoElementND[] out = processor.processAlgebraCommandNoExceptionHandling(
                line, false, new ThrowingErrorHandler(line), false, null);
        if (out != null && out.length == 2 && isMultiOutput(definition)) {
            return runMulti(line, expected, out);
        }
        // A transformed segment or ray with labelled end points also creates
        // the image points (see `transformedOutputs`); the segment or ray is
        // the first output. Any other object gives exactly one output.
        if (out == null || out.length != transformedOutputs(transformed, definition)) {
            throw new IllegalStateException("unexpected number of objects from: " + line);
        }
        GeoElement geo = out[0].toGeoElement();
        boolean labelled = kernel.lookupLabel(label) == geo;
        boolean waiting = !geo.isLabelSet() && !geo.isDefined()
                && label.equals(geo.getLabelSimple());
        if (!labelled && !waiting) {
            throw new IllegalStateException("GeoGebra labelled '" + line + "' as "
                    + geo.getLabelSimple());
        }
        String actual = typeOf(geo);
        if (!expected.equals(actual)) {
            throw new IllegalStateException("expected a " + expected + " from '" + line
                    + "', got " + geo.getClass().getSimpleName());
        }
        boolean free = (expected.equals("point") || expected.equals("vector"))
                && POINT.matcher(m.group(2).trim()).matches()
                || expected.equals("number") && NUMBER.matcher(m.group(2).trim()).matches();
        if (free != geo.isIndependent()) {
            throw new IllegalStateException("unexpected dependency state for: " + line);
        }
        objects.put(label, geo);
        return geo;
    }

    /**
     * Records the objects of a command that creates several (for example
     * the two tangents of `Tangent(P, c)`) under the labels GeoGebra gave
     * them. GeoGebra labels only the outputs that are defined when the line
     * runs (two or more get indexed labels such as `t_{1}`, a single one the
     * plain label); the others stay unlabelled and are not recorded. At least
     * one output must be labelled.
     */
    private GeoElement runMulti(String line, String expected, GeoElementND[] out) {
        GeoElement first = null;
        for (GeoElementND nd : out) {
            GeoElement geo = nd.toGeoElement();
            if (!geo.isLabelSet()) {
                continue;
            }
            String label = geo.getLabelSimple();
            if (kernel.lookupLabel(label) != geo) {
                throw new IllegalStateException("GeoGebra labelled an object of '" + line
                        + "' as " + label + " but cannot look it up");
            }
            if (objects.containsKey(label)) {
                throw new IllegalArgumentException("label used twice: " + label);
            }
            String actual = typeOf(geo);
            if (!expected.equals(actual)) {
                throw new IllegalStateException("expected a " + expected + " from '" + line
                        + "', got " + geo.getClass().getSimpleName());
            }
            objects.put(label, geo);
            if (first == null) {
                first = geo;
            }
        }
        if (first == null) {
            throw new IllegalStateException("no object of '" + line
                    + "' is labelled; at least one output must be defined when it is created");
        }
        return first;
    }

    /** Moves a free point or a free vector and updates everything that depends on it. */
    void move(String label, double x, double y) {
        GeoElement geo = lookup(label);
        if (geo instanceof GeoVector && geo.isIndependent()) {
            GeoVector v = (GeoVector) geo;
            v.setCoords(x, y, 0.0);
            v.updateCascade();
            return;
        }
        if (!(geo instanceof GeoPoint) || !geo.isIndependent()) {
            throw new IllegalArgumentException(label + " is not a free point");
        }
        GeoPoint p = (GeoPoint) geo;
        p.setCoords(x, y, 1.0);
        p.updateCascade();
    }

    /** Sets a free number and updates everything that depends on it. */
    void setNumber(String label, double value) {
        GeoElement geo = lookup(label);
        if (!(geo instanceof GeoNumeric) || !geo.isIndependent()) {
            throw new IllegalArgumentException(label + " is not a free number");
        }
        GeoNumeric n = (GeoNumeric) geo;
        n.setValue(value);
        if (Double.doubleToRawLongBits(n.getDouble()) != Double.doubleToRawLongBits(value)) {
            throw new IllegalStateException("GeoGebra stored " + n.getDouble()
                    + " instead of " + value + " in " + label);
        }
        n.updateCascade();
    }

    /** Records every scripted object, in script order. */
    Map<String, Object> snapshot() {
        Map<String, Object> snap = new LinkedHashMap<>();
        for (Map.Entry<String, GeoElement> e : objects.entrySet()) {
            snap.put(e.getKey(), describe(e.getValue()));
        }
        return snap;
    }

    private GeoElement lookup(String label) {
        GeoElement geo = objects.get(label);
        if (geo == null) {
            throw new IllegalArgumentException("unknown label " + label);
        }
        return geo;
    }

    /** The schema type of a GeoGebra object; the class decides, not the state. */
    static String typeOf(GeoElement geo) {
        if (geo instanceof GeoSegment) {
            return "segment";
        }
        if (geo instanceof GeoRay) {
            return "ray";
        }
        if (geo instanceof GeoLine) {
            return "line";
        }
        if (geo instanceof GeoConic) {
            return "circle";
        }
        if (geo instanceof GeoPoint) {
            return "point";
        }
        if (geo instanceof GeoNumeric) {
            return "number";
        }
        if (geo instanceof GeoVector) {
            return "vector";
        }
        throw new IllegalStateException("unsupported object " + geo.getClass().getSimpleName());
    }

    /**
     * The schema type of a transformation (the image has the kind of the
     * transformed object); null when the definition is not a transformation.
     */
    private String transformedType(String definition) {
        Matcher m = COMMAND.matcher(definition);
        if (!m.matches()) {
            return null;
        }
        switch (m.group(1)) {
            case "Mirror":
            case "Reflect":
            case "Rotate":
            case "Dilate":
            case "Shear":
            case "Stretch":
            case "Translate":
                break;
            default:
                return null;
        }
        return typeOf(transformedSource(definition));
    }

    /** The object a transformation command transforms: its first argument. */
    private GeoElement transformedSource(String definition) {
        String inner = definition.substring(definition.indexOf('(') + 1);
        String first = inner.split("[,)]")[0].trim();
        GeoElement source = objects.get(first);
        if (source == null) {
            throw new IllegalArgumentException("unknown transformed object: " + definition);
        }
        return source;
    }

    /**
     * How many objects a definition creates. A transformed segment creates
     * the images of its two end points too; a transformed ray creates the
     * images of the points it was made from: two for `Ray(A, B)`, one (the
     * start point) for `Ray(A, v)`. Everything else creates one object.
     */
    private int transformedOutputs(String transformed, String definition) {
        if ("segment".equals(transformed)) {
            return 3;
        }
        if ("ray".equals(transformed)) {
            AlgoElement parent = transformedSource(definition).getParentAlgorithm();
            if (parent instanceof AlgoJoinPointsRay) {
                return 3;
            }
            if (parent instanceof AlgoRayPointVector) {
                return 2;
            }
        }
        return 1;
    }

    /**
     * Whether the definition is a command that creates two lines by design:
     * `Tangent` and `AngularBisector` (`AngleBisector`) of two lines. Any
     * other command must create exactly one object (see `run`).
     */
    private static boolean isMultiOutput(String definition) {
        Matcher m = COMMAND.matcher(definition);
        if (!m.matches()) {
            return false;
        }
        switch (m.group(1)) {
            case "Tangent":
            case "AngularBisector":
            case "AngleBisector":
                return true;
            default:
                return false;
        }
    }

    /**
     * The schema type a definition must produce; rejects unsupported input.
     * A coordinate pair is a vector when the first character of the label is
     * lower case by `StringUtil.isLowerCase` and a point otherwise
     * (`AlgebraProcessor.isVectorLabel`).
     */
    private static String expectedType(String label, String definition) {
        if (POINT.matcher(definition).matches()) {
            return StringUtil.isLowerCase(label.charAt(0)) ? "vector" : "point";
        }
        if (NUMBER.matcher(definition).matches()) {
            return "number";
        }
        Matcher m = COMMAND.matcher(definition);
        if (!m.matches()) {
            throw new IllegalArgumentException("unsupported definition: " + definition);
        }
        switch (m.group(1)) {
            case "Line":
            case "PerpendicularLine":
            case "Diameter":
            case "OrthogonalLine":
            case "LineBisector":
            case "PerpendicularBisector":
            case "AngularBisector":
            case "AngleBisector":
            case "Tangent":
            case "Polar":
                return "line";
            case "Segment":
                return "segment";
            case "Midpoint":
            case "Center":
            case "Intersect":
                return "point";
            case "Distance":
            case "Length":
            case "Radius":
            case "Circumference":
            case "Perimeter":
            case "Area":
            case "Slope":
            case "AffineRatio":
            case "CrossRatio":
            case "Angle":
                return "number";
            case "Circle":
            case "Incircle":
                return "circle";
            case "Vector":
            case "Direction":
            case "UnitVector":
            case "OrthogonalVector":
            case "PerpendicularVector":
            case "UnitOrthogonalVector":
            case "UnitPerpendicularVector":
                return "vector";
            case "Ray":
                return "ray";
            default:
                throw new IllegalArgumentException("unsupported command: " + definition);
        }
    }

    private static Map<String, Object> describe(GeoElement geo) {
        String type = typeOf(geo);
        Map<String, Object> obj = new LinkedHashMap<>();
        obj.put("type", type);
        Map<String, Object> fields = geo.isDefined() ? fields(geo, type) : null;
        if (fields != null && fields.values().stream()
                .allMatch(v -> Double.isFinite((Double) v))) {
            obj.put("defined", true);
            obj.putAll(fields);
        } else {
            obj.put("defined", false);
        }
        return obj;
    }

    /** Numeric fields of a defined object; null when GeoGebra has none. */
    private static Map<String, Object> fields(GeoElement geo, String type) {
        Map<String, Object> f = new LinkedHashMap<>();
        switch (type) {
            case "point": {
                GeoPoint p = (GeoPoint) geo;
                if (p.isInfinite()) {
                    return null;
                }
                f.put("x", p.getInhomX());
                f.put("y", p.getInhomY());
                return f;
            }
            case "line": {
                GeoLine l = (GeoLine) geo;
                f.put("a", l.x);
                f.put("b", l.y);
                f.put("c", l.z);
                return f;
            }
            case "segment": {
                GeoSegment s = (GeoSegment) geo;
                GeoPoint p = s.getStartPoint();
                GeoPoint q = s.getEndPoint();
                if (p.isInfinite() || q.isInfinite()) {
                    return null;
                }
                f.put("x1", p.getInhomX());
                f.put("y1", p.getInhomY());
                f.put("x2", q.getInhomX());
                f.put("y2", q.getInhomY());
                return f;
            }
            case "circle": {
                GeoConic c = (GeoConic) geo;
                int t = c.getType();
                // A circle whose centre is a point at infinity becomes a pair
                // of parallel lines with infinite half-axes; the infinite
                // radius makes it `defined: false` like any non-finite value.
                boolean infinite = !Double.isFinite(c.getHalfAxis(0));
                if (t != GeoConic.CONIC_CIRCLE && t != GeoConic.CONIC_SINGLE_POINT
                        && !infinite) {
                    throw new IllegalStateException("circle " + geo.getLabelSimple()
                            + " became a conic of type " + t);
                }
                Coords mid = c.getMidpoint2D();
                f.put("cx", mid.getX());
                f.put("cy", mid.getY());
                f.put("r", c.getHalfAxis(0));
                return f;
            }
            case "number":
                f.put("value", ((GeoNumeric) geo).getDouble());
                return f;
            case "vector": {
                GeoVector v = (GeoVector) geo;
                f.put("x", v.getX());
                f.put("y", v.getY());
                return f;
            }
            case "ray": {
                GeoRay r = (GeoRay) geo;
                GeoPoint p = r.getStartPoint();
                if (p == null || p.isInfinite()) {
                    return null;
                }
                f.put("x", p.getInhomX());
                f.put("y", p.getInhomY());
                f.put("a", r.x);
                f.put("b", r.y);
                f.put("c", r.z);
                return f;
            }
            default:
                throw new IllegalStateException(type);
        }
    }

    /** Turns every GeoGebra error report into an exception. */
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
