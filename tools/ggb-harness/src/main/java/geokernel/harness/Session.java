package geokernel.harness;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.algos.AlgoElement;
import org.geogebra.common.kernel.algos.AlgoJoinPointsRay;
import org.geogebra.common.kernel.algos.AlgoPointOnPath;
import org.geogebra.common.kernel.algos.AlgoPointInRegion;
import org.geogebra.common.kernel.algos.AlgoRayPointVector;
import org.geogebra.common.kernel.commands.AlgebraProcessor;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoConicPart;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoFunction;
import org.geogebra.common.kernel.geos.GeoLine;
import org.geogebra.common.kernel.geos.GeoList;
import org.geogebra.common.kernel.geos.GeoBoolean;
import org.geogebra.common.kernel.geos.GeoAngle;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoPolyLine;
import org.geogebra.common.kernel.geos.GeoPolygon;
import org.geogebra.common.kernel.geos.GeoRay;
import org.geogebra.common.kernel.geos.GeoSegment;
import org.geogebra.common.kernel.geos.GeoVector;
import org.geogebra.common.kernel.kernelND.GeoConicND;
import org.geogebra.common.kernel.kernelND.GeoConicNDConstants;
import org.geogebra.common.kernel.kernelND.GeoElementND;
import org.geogebra.common.kernel.kernelND.GeoPointND;
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
            Pattern.compile("^([A-Za-z]+(?:_\\{[A-Za-z0-9]+\\})?)(\\(x\\))? = (.+)$");
    /** A label as the script lines write it, to find the objects an expression reads. */
    private static final Pattern LABEL = Pattern.compile("[A-Za-z]+(?:_\\{[A-Za-z0-9]+\\})?");
    /** Where a function is sampled, in this order (see golden/SCHEMA.md). */
    static final double[] SAMPLES = {-8, -4, -2, -1, -0.5, 0, 0.5, 1, 2, 4, 8};
    private static final Pattern NUMBER = Pattern.compile("^-?\\d+(\\.\\d+)?$");
    private static final Pattern POINT =
            Pattern.compile("^\\(-?\\d+(\\.\\d+)?, -?\\d+(\\.\\d+)?\\)$");
    private static final Pattern CALL = Pattern.compile("(?<![A-Za-z0-9_])([A-Z][A-Za-z0-9]*)\\s*\\(");
    private static final Pattern COMMAND = Pattern.compile("^([A-Za-z]+)\\[?\\(.*\\)$");
    /** The variable `x` of a function: not part of a longer name, not a call. */
    private static final Pattern VARIABLE_X = Pattern.compile("(?<![A-Za-z0-9_])x(?![A-Za-z0-9_(])");

    private final AppCommon app;
    private final Kernel kernel;
    private final AlgebraProcessor processor;
    /**
     * Scripted objects by label. GeoGebra gives an object that is undefined
     * when it is created no label until it becomes defined, so snapshots hold
     * on to the objects themselves instead of looking labels up.
     */
    private final Map<String, GeoElement> objects = new LinkedHashMap<>();
    /**
     * The scripted objects recorded as general conics (type `conic`): those
     * made by `Conic`, `Ellipse`, `Hyperbola` or `Parabola`, and the images
     * of such objects under a transformation. Every other `GeoConic` is a
     * `circle`.
     */
    private final Set<GeoElement> conics = Collections.newSetFromMap(new IdentityHashMap<>());

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
        boolean functionLine = m.group(2) != null;
        String definition = m.group(3).trim();
        Matcher references = LABEL.matcher(definition);
        while (references.find()) {
            String reference = references.group();
            if (reference.equals("xAxis") || reference.equals("yAxis")) {
                objects.putIfAbsent(reference, kernel.lookupLabel(reference));
            }
        }
        String transformed = functionLine ? null : transformedType(definition);
        String expected = functionLine ? "function"
                : transformed != null ? transformed : expectedType(label, definition);
        if (objects.containsKey(label) || kernel.lookupLabel(label) != null) {
            throw new IllegalArgumentException("label used twice: " + label);
        }
        GeoElementND[] out = processor.processAlgebraCommandNoExceptionHandling(
                line, false, new ThrowingErrorHandler(line), false, null);
        if ((out == null || out.length == 0) && definition.startsWith("Segment(")
                && definition.chars().filter(ch -> ch == ',').count() == 2) {
            return null;
        }
        if (out != null && out.length == 2 && definition.startsWith("Segment(")) {
            return runFixedSegment(line, label, out);
        }
        if (out != null && out.length == 4
                && (definition.startsWith("OrthogonalLine(")
                || definition.startsWith("PerpendicularLine("))) {
            return runOrthogonalLine(line, label, out);
        }
        if (out != null && out.length > 0
                && (definition.startsWith("Difference(")
                || definition.startsWith("IntersectPath("))) {
            return runClipping(line, label, out);
        }
        if (out != null && out.length == 2 && definition.startsWith("Angle(")
                && out[1].toGeoElement() instanceof GeoPoint) {
            return runFixedAngle(line, label, out);
        }
        if (out != null && out.length == 2 && isMultiOutput(definition)) {
            return runMulti(line, expected, out);
        }
        if (out != null && out.length >= 1 && isPerVertex(definition)) {
            return runMulti(line, expected, out);
        }
        if (out != null && out.length >= 1 && (definition.startsWith("Polygon(")
                || definition.startsWith("RigidPolygon("))) {
            if (definition.startsWith("RigidPolygon(")) {
                refreshPoints();
            }
            return runPolygon(line, label, out);
        }
        // A transformed segment or ray with labelled end points also creates
        // the image points (see `transformedOutputs`); the segment or ray is
        // the first output. Any other object gives exactly one output.
        if (out == null || out.length != transformedOutputs(transformed, definition)) {
            throw new IllegalStateException("unexpected number of objects from: " + line);
        }
        GeoElement geo = out[0].toGeoElement();
        if (geo instanceof GeoAngle && definition.startsWith("Angle(")
                && objects.containsValue(geo)) {
            return geo;
        }
        boolean labelled = kernel.lookupLabel(label) == geo;
        boolean waiting = !geo.isLabelSet() && !geo.isDefined()
                && label.equals(geo.getLabelSimple());
        if (!labelled && !waiting) {
            throw new IllegalStateException("GeoGebra labelled '" + line + "' as "
                    + geo.getLabelSimple());
        }
        String actual = "conic".equals(expected) && geo instanceof GeoConic ? "conic" : schemaType(geo);
        boolean expression = expected == null;
        if (expression) {
            expected = expressionType(label, actual, line);
        }
        if (!expected.equals(actual)) {
            throw new IllegalStateException("expected a " + expected + " from '" + line
                    + "', got " + geo.getClass().getSimpleName());
        }
        boolean free = expression || functionLine
                ? !readsObjects(definition) && !callsCommand(definition)
                : (expected.equals("point") || expected.equals("vector"))
                        && POINT.matcher(definition).matches()
                        || expected.equals("number") && NUMBER.matcher(definition).matches()
                        || expected.equals("angle") && definition.startsWith("Angle(")
                                && definition.endsWith("°)");
        if (free != geo.isIndependent()) {
            throw new IllegalStateException("unexpected dependency state for: " + line);
        }
        objects.put(label, geo);
        if (actual.equals("conic")) {
            conics.add(geo);
        }
        return geo;
    }

    private GeoElement runFixedAngle(String line, String label, GeoElementND[] out) {
        GeoElement angle = out[0].toGeoElement();
        GeoElement point = out[1].toGeoElement();
        if (!(angle instanceof GeoAngle) || !label.equals(angle.getLabelSimple())) {
            throw new IllegalStateException("unexpected outputs from: " + line);
        }
        if (point.isLabelSet()) {
            String name = point.getLabelSimple();
            if (kernel.lookupLabel(name) != point || objects.putIfAbsent(name, point) != null) {
                throw new IllegalStateException("unexpected rotated point " + name + ": " + line);
            }
        }
        objects.put(label, angle);
        return angle;
    }

    private GeoElement runClipping(String line, String label, GeoElementND[] out) {
        GeoElement first = out[0].toGeoElement();
        if (!(first instanceof GeoPolygon) && !(first instanceof GeoSegment)) {
            throw new IllegalStateException("unexpected clipping result from: " + line);
        }
        for (GeoElementND element : out) {
            GeoElement geo = element.toGeoElement();
            if (geo.isLabelSet()) {
                String name = geo.getLabelSimple();
                if (kernel.lookupLabel(name) != geo || objects.putIfAbsent(name, geo) != null) {
                    throw new IllegalStateException("unexpected output label " + name + " from: " + line);
                }
            }
        }
        if (!first.isLabelSet()) {
            objects.put(label, first);
        }
        return first;
    }

    private GeoElement runFixedSegment(String line, String label, GeoElementND[] out) {
        GeoElement segment = out[0].toGeoElement();
        GeoElement endpoint = out[1].toGeoElement();
        if (!(segment instanceof GeoSegment) || !(endpoint instanceof GeoPoint)
                || !label.equals(segment.getLabelSimple())) {
            throw new IllegalStateException("unexpected outputs from: " + line);
        }
        objects.put(label, segment);
        if (endpoint.isLabelSet()) {
            objects.put(endpoint.getLabelSimple(), endpoint);
        }
        return segment;
    }

    private GeoElement runOrthogonalLine(String line, String label, GeoElementND[] out) {
        GeoElement first = out[0].toGeoElement();
        if (!(first instanceof GeoLine) || !label.equals(first.getLabelSimple())) {
            throw new IllegalStateException("unexpected conic normal from: " + line);
        }
        objects.put(label, first);
        return first;
    }

    private void refreshPoints() {
        objects.replaceAll((label, geo) -> {
            GeoElement replacement = kernel.lookupLabel(label);
            return geo instanceof GeoPoint && replacement instanceof GeoPoint ? replacement : geo;
        });
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
            String actual = schemaType(geo);
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

    /**
     * Records a polygon and the objects `Polygon` creates with it: its sides
     * and, for a regular polygon, its new vertices, under the labels GeoGebra
     * gave them. The polygon must get the script label, or be undefined and
     * wait for it like any other object (see `run`). Outputs GeoGebra left
     * unlabelled are not recorded; every labelled one must be a segment or a
     * point.
     */
    private GeoElement runPolygon(String line, String label, GeoElementND[] out) {
        GeoElement poly = out[0].toGeoElement();
        boolean labelled = kernel.lookupLabel(label) == poly;
        boolean waiting = !poly.isLabelSet() && !poly.isDefined()
                && label.equals(poly.getLabelSimple());
        if (!(poly instanceof GeoPolygon) || !labelled && !waiting) {
            throw new IllegalStateException("'" + line + "' did not create polygon " + label);
        }
        objects.put(label, poly);
        for (int i = 1; i < out.length; i++) {
            GeoElement geo = out[i].toGeoElement();
            if (!geo.isLabelSet()) {
                continue;
            }
            String name = geo.getLabelSimple();
            if (kernel.lookupLabel(name) != geo) {
                throw new IllegalStateException("GeoGebra labelled an object of '" + line
                        + "' as " + name + " but cannot look it up");
            }
            if (objects.containsKey(name)) {
                throw new IllegalArgumentException("label used twice: " + name);
            }
            String type = typeOf(geo);
            if (!type.equals("segment") && !type.equals("point")) {
                throw new IllegalStateException("'" + line + "' created a " + type);
            }
            objects.put(name, geo);
        }
        return poly;
    }

    /**
     * Moves a free point or a free vector and updates everything that depends
     * on it. A point on a path made by `Point(path)` (without a parameter) is
     * moved the same way: `setCoords` projects it onto its path and stores its
     * path parameter (`GeoPoint.setCoords` calls `Path.pointChanged`).
     */
    void move(String label, double x, double y) {
        GeoElement geo = lookup(label);
        if (isMovableOnPath(geo)) {
            GeoPoint p = (GeoPoint) geo;
            p.setCoords(x, y, 1.0);
            p.updateCascade();
            return;
        }
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

    /** Whether `geo` is a point on a path that the user can drag: `Point(path)`. */
    private static boolean isMovableOnPath(GeoElement geo) {
        AlgoElement parent = geo.getParentAlgorithm();
        return geo instanceof GeoPoint && (parent instanceof AlgoPointInRegion
                || parent instanceof AlgoPointOnPath
                && ((AlgoPointOnPath) parent).isChangeable(geo));
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
            snap.put(e.getKey(), describe(e.getValue(), schemaType(e.getValue())));
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

    /**
     * The schema type an arithmetic expression must produce: a number, a
     * function (it reads `x`), a boolean, or a point or vector, which
     * GeoGebra decides by the label (`AlgebraProcessor.isVectorLabel`) and
     * not by the operands.
     */
    private static String expressionType(String label, String actual, String line) {
        switch (actual) {
            case "number":
            case "angle":
            case "function":
            case "boolean":
            case "list":
                return actual;
            case "point":
            case "vector":
                return StringUtil.isLowerCase(label.charAt(0)) ? "vector" : "point";
            default:
                throw new IllegalStateException("an expression made a " + actual + ": " + line);
        }
    }

    /** Whether a definition names a scripted object (then GeoGebra makes it dependent). */
    /**
     * Whether the definition calls a command such as {@code Mod(7, 3)}: an
     * upper-case name before a bracket that is not an object. The command
     * makes a helper object, so the result is dependent even on constants.
     */
    private boolean callsCommand(String definition) {
        Matcher m = CALL.matcher(definition);
        while (m.find()) {
            if (objects.containsKey(m.group(1))) {
                continue;
            }
            if (m.group(1).equals("If") && conditionsReadX(definition, m.end())) {
                // `CmdIf.specialFunction`: an operation of the function
                continue;
            }
            return true;
        }
        return false;
    }

    /**
     * Whether a condition of the `If` whose arguments start at `open` (the
     * index after its bracket) reads the variable `x`: its arguments at
     * even positions, except a trailing else branch.
     */
    private static boolean conditionsReadX(String definition, int open) {
        List<String> args = new ArrayList<>();
        int depth = 0;
        int start = open;
        for (int i = open; i < definition.length(); i++) {
            char ch = definition.charAt(i);
            if (ch == '(' || ch == '{' || ch == '[') {
                depth++;
            } else if (ch == ')' || ch == '}' || ch == ']') {
                if (depth == 0) {
                    args.add(definition.substring(start, i));
                    break;
                }
                depth--;
            } else if (ch == ',' && depth == 0) {
                args.add(definition.substring(start, i));
                start = i + 1;
            }
        }
        for (int k = 0; k + 1 < args.size(); k += 2) {
            if (VARIABLE_X.matcher(args.get(k)).find()) {
                return true;
            }
        }
        return false;
    }

    private boolean readsObjects(String definition) {
        Matcher m = LABEL.matcher(definition);
        while (m.find()) {
            if (objects.containsKey(m.group())) {
                return true;
            }
        }
        return false;
    }

    /**
     * Whether the definition is exactly one call `Name(...)`: the bracket
     * after the name closes at the end. `Distance(A, B) / (2)` is not.
     */
    private static boolean isSingleCall(String definition) {
        Matcher m = COMMAND.matcher(definition);
        if (!m.matches()) {
            return false;
        }
        int depth = 0;
        for (int i = m.group(1).length(); i < definition.length(); i++) {
            char ch = definition.charAt(i);
            if (ch == '(' || ch == '[') {
                depth++;
            } else if (ch == ')' || ch == ']') {
                depth--;
                if (depth == 0) {
                    return i == definition.length() - 1;
                }
            }
        }
        return false;
    }

    /** The schema type of a scripted object: `conic` for a general conic, else {@link #typeOf}. */
    private String schemaType(GeoElement geo) {
        return conics.contains(geo) ? "conic" : typeOf(geo);
    }

    /** The schema type of a GeoGebra object; the class decides, not the state. */
    static String typeOf(GeoElement geo) {
        if (geo instanceof GeoList) {
            return "list";
        }
        if (geo instanceof GeoPolygon) {
            return "polygon";
        }
        if (geo instanceof GeoPolyLine) {
            return "polyline";
        }
        if (geo instanceof GeoSegment) {
            return "segment";
        }
        if (geo instanceof GeoRay) {
            return "ray";
        }
        if (geo instanceof GeoLine) {
            return "line";
        }
        // an arc or sector is a GeoConic too
        if (geo instanceof GeoConicPart) {
            return ((GeoConicPart) geo).getConicPartType()
                    == GeoConicNDConstants.CONIC_PART_SECTOR ? "sector" : "arc";
        }
        if (geo instanceof GeoConic) {
            return "circle";
        }
        if (geo instanceof GeoPoint) {
            return "point";
        }
        if (geo instanceof GeoAngle) {
            return "angle";
        }
        if (geo instanceof GeoNumeric) {
            return "number";
        }
        if (geo instanceof GeoBoolean) {
            return "boolean";
        }
        if (geo instanceof GeoVector) {
            return "vector";
        }
        if (geo instanceof GeoFunction) {
            return "function";
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
        return schemaType(transformedSource(definition));
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
     * Whether the definition is a command that creates two objects by design:
     * `Tangent` and `AngularBisector` (`AngleBisector`) of two lines, and
     * `Focus`, `Axes` and `Directrix` of a conic. Any other command must
     * create exactly one object (see `run`).
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
            case "Focus":
            case "Axes":
            case "Directrix":
                return true;
            default:
                return false;
        }
    }

    /**
     * Whether the definition creates one object per vertex of a polygon or
     * polyline: `Vertex(p)`, `InteriorAngles(p)` and `Angle(p)` (one argument
     * without a comma, a polygon for `Angle`). They are labelled like the
     * outputs of `runMulti`.
     */
    private boolean isPerVertex(String definition) {
        Matcher m = COMMAND.matcher(definition);
        if (!m.matches() || definition.contains(",")) {
            return false;
        }
        switch (m.group(1)) {
            case "Vertex":
            case "InteriorAngles":
                return true;
            case "Angle":
                return objects.get(definition.substring(definition.indexOf('(') + 1,
                        definition.length() - 1).trim()) instanceof GeoPolygon;
            default:
                return false;
        }
    }

    /**
     * The schema type a definition must produce; rejects unsupported input.
     * A coordinate pair is a vector when the first character of the label is
     * lower case by `StringUtil.isLowerCase` and a point otherwise
     * (`AlgebraProcessor.isVectorLabel`). Null for an arithmetic expression
     * (anything that is not a literal or one call of a known command); its
     * type is checked by `expressionType`.
     */
    private String expectedType(String label, String definition) {
        if (POINT.matcher(definition).matches()) {
            return StringUtil.isLowerCase(label.charAt(0)) ? "vector" : "point";
        }
        if (NUMBER.matcher(definition).matches()) {
            return "number";
        }
        if (!isSingleCall(definition)) {
            return null;
        }
        Matcher m = COMMAND.matcher(definition);
        if (!m.matches()) {
            throw new IllegalStateException("not a call: " + definition);
        }
        String command = m.group(1);
        String arguments = definition.substring(definition.indexOf('(') + 1, definition.length() - 1);
        GeoElement first = objects.get(arguments.split(",", 2)[0].trim());
        switch (command) {
            case "Polar":
                return first instanceof GeoLine ? "point" : "line";
            case "Center":
            case "Midpoint":
                return VARIABLE_X.matcher(arguments).find()
                        || first instanceof GeoFunction && ((GeoFunction) first).isBooleanFunction()
                        ? "number" : "point";
            case "Line":
            case "PerpendicularLine":
            case "Diameter":
            case "ConjugateDiameter":
            case "OrthogonalLine":
            case "LineBisector":
            case "PerpendicularBisector":
            case "AngularBisector":
            case "AngleBisector":
            case "Tangent":
            case "Axes":
            case "MajorAxis":
            case "FirstAxis":
            case "MinorAxis":
            case "SecondAxis":
            case "Directrix":
                return "line";
            case "Segment":
                return "segment";
            case "Intersect":
            case "ClosestPoint":
            case "Trilinear":
            case "Centroid":
            case "Vertex":
            case "Focus":
            case "Point":
            case "PointIn":
            case "TriangleCenter":
                return "point";
            case "Polygon":
            case "RigidPolygon":
                return "polygon";
            case "PolyLine":
                return "polyline";
            case "AreCollinear":
            case "AreParallel":
            case "ArePerpendicular":
            case "AreConcurrent":
            case "AreConcyclic":
            case "AreEqual":
            case "AreCongruent":
            case "IsTangent":
                return "boolean";
            case "Distance":
            case "Length":
            case "Radius":
            case "Circumference":
            case "Perimeter":
            case "Area":
            case "Slope":
            case "AffineRatio":
            case "CrossRatio":
            case "FirstAxisLength":
            case "SemiMajorAxisLength":
            case "SecondAxisLength":
            case "SemiMinorAxisLength":
            case "Eccentricity":
            case "LinearEccentricity":
            case "Excentricity":
            case "Parameter":
            case "PathParameter":
                return "number";
            case "Angle":
            case "InteriorAngles":
                return "angle";
            case "Circle":
            case "Incircle":
                return "circle";
            case "Conic":
            case "Ellipse":
            case "Hyperbola":
            case "Parabola":
                return "conic";
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
            case "CircularArc":
            case "CircleArc":
            case "CircumcircularArc":
            case "CircumcircleArc":
            case "Semicircle":
            case "Arc":
                return "arc";
            case "CircularSector":
            case "CircleSector":
            case "CircumcircularSector":
            case "CircumcircleSector":
            case "Sector":
                return "sector";
            default:
                // a mathematical function such as `sqrt(a)` or `x(A)`
                return null;
        }
    }

    private static Map<String, Object> describe(GeoElement geo, String type) {
        Map<String, Object> obj = new LinkedHashMap<>();
        obj.put("type", type);
        if (geo instanceof GeoBoolean) {
            // GeoBoolean.isDefined is false only for the prover's "unknown"
            // answer of AreCongruent; a boolean has no numeric fields.
            if (geo.isDefined()) {
                obj.put("defined", true);
                obj.put("value", ((GeoBoolean) geo).getBoolean());
            } else {
                obj.put("defined", false);
            }
            return obj;
        }
        if (geo instanceof GeoList) {
            // the elements in order, each described like an object
            GeoList list = (GeoList) geo;
            obj.put("defined", list.isDefined());
            if (list.isDefined()) {
                List<Object> elements = new ArrayList<>();
                for (int i = 0; i < list.size(); i++) {
                    elements.add(describe(list.get(i), typeOf(list.get(i))));
                }
                obj.put("elements", elements);
            }
            return obj;
        }
        if (type.equals("function")) {
            GeoFunction f = (GeoFunction) geo;
            obj.put("defined", f.isDefined());
            if (f.isDefined()) {
                List<Object> samples = new ArrayList<>();
                for (double x : SAMPLES) {
                    double v = f.isBooleanFunction() ? (f.evaluateBoolean(x) ? 1.0 : 0.0) : f.value(x);
                    samples.add(Arrays.asList(x, Double.isFinite(v) ? (Object) v : null));
                }
                obj.put("samples", samples);
            }
            return obj;
        }
        Map<String, Object> fields = geo.isDefined() ? fields(geo, type) : null;
        if (fields != null && fields.values().stream().allMatch(Session::finite)) {
            obj.put("defined", true);
            obj.putAll(fields);
        } else {
            obj.put("defined", false);
        }
        return obj;
    }

    /** Whether a field is finite: a number, or a list of finite fields or nulls; a name is. */
    private static boolean finite(Object v) {
        if (v == null || v instanceof String) {
            return true;
        }
        if (v instanceof List) {
            return ((List<?>) v).stream().allMatch(Session::finite);
        }
        return Double.isFinite((Double) v);
    }

    /** A vertex `[x, y]`; null for an undefined point or a point at infinity. */
    private static List<Object> vertex(GeoPointND p) {
        GeoPoint q = (GeoPoint) p;
        if (!q.isDefined() || q.isInfinite()) {
            return null;
        }
        List<Object> v = new ArrayList<>();
        v.add(q.getInhomX());
        v.add(q.getInhomY());
        return v;
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
            case "conic":
                return conicFields((GeoConicND) geo);
            case "arc":
            case "sector":
                return arcFields((GeoConicPart) geo);
            case "angle":
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
            case "polygon": {
                GeoPolygon poly = (GeoPolygon) geo;
                List<Object> vertices = new ArrayList<>();
                for (GeoPointND p : poly.getPoints()) {
                    List<Object> v = vertex(p);
                    if (v == null) {
                        return null;
                    }
                    vertices.add(v);
                }
                f.put("vertices", vertices);
                f.put("area", poly.getArea());
                f.put("signed_area", poly.getAreaWithSign());
                return f;
            }
            case "polyline": {
                GeoPolyLine poly = (GeoPolyLine) geo;
                List<Object> vertices = new ArrayList<>();
                for (GeoPointND p : poly.getPointsND()) {
                    vertices.add(vertex(p));
                }
                f.put("vertices", vertices);
                f.put("length", poly.getLength());
                return f;
            }
            default:
                throw new IllegalStateException(type);
        }
    }

    /** `[x, y]`. */
    private static List<Object> pair(double x, double y) {
        return Arrays.asList(x, y);
    }

    /**
     * The fields of a general conic: its type, its raw matrix as GeoGebra
     * holds it, and the parts its type has (see golden/SCHEMA.md).
     */
    private static Map<String, Object> conicFields(GeoConicND c) {
        Map<String, Object> f = new LinkedHashMap<>();
        int t = c.getType();
        String name;
        switch (t) {
            case GeoConicNDConstants.CONIC_SINGLE_POINT: name = "single_point"; break;
            case GeoConicNDConstants.CONIC_INTERSECTING_LINES: name = "intersecting_lines"; break;
            case GeoConicNDConstants.CONIC_ELLIPSE: name = "ellipse"; break;
            case GeoConicNDConstants.CONIC_CIRCLE: name = "circle"; break;
            case GeoConicNDConstants.CONIC_HYPERBOLA: name = "hyperbola"; break;
            case GeoConicNDConstants.CONIC_EMPTY: name = "empty"; break;
            case GeoConicNDConstants.CONIC_DOUBLE_LINE: name = "double_line"; break;
            case GeoConicNDConstants.CONIC_PARALLEL_LINES: name = "parallel_lines"; break;
            case GeoConicNDConstants.CONIC_PARABOLA: name = "parabola"; break;
            default:
                throw new IllegalStateException("conic of unsupported type " + t);
        }
        f.put("conic_type", name);
        List<Object> matrix = new ArrayList<>();
        for (double v : c.getMatrix()) {
            matrix.add(v);
        }
        f.put("matrix", matrix);
        Coords mid = c.getMidpoint2D();
        List<Object> eigenvectors = Arrays.asList(
                pair(c.getEigenvec(0).getX(), c.getEigenvec(0).getY()),
                pair(c.getEigenvec(1).getX(), c.getEigenvec(1).getY()));
        switch (t) {
            case GeoConicNDConstants.CONIC_CIRCLE:
            case GeoConicNDConstants.CONIC_ELLIPSE:
            case GeoConicNDConstants.CONIC_HYPERBOLA:
                f.put("midpoint", pair(mid.getX(), mid.getY()));
                f.put("half_axes", pair(c.getHalfAxis(0), c.getHalfAxis(1)));
                f.put("eigenvectors", eigenvectors);
                break;
            case GeoConicNDConstants.CONIC_PARABOLA:
                f.put("vertex", pair(mid.getX(), mid.getY()));
                f.put("p", c.p);
                f.put("eigenvectors", eigenvectors);
                break;
            case GeoConicNDConstants.CONIC_SINGLE_POINT:
                f.put("point", pair(mid.getX(), mid.getY()));
                break;
            case GeoConicNDConstants.CONIC_INTERSECTING_LINES:
            case GeoConicNDConstants.CONIC_PARALLEL_LINES:
            case GeoConicNDConstants.CONIC_DOUBLE_LINE: {
                List<Object> lines = new ArrayList<>();
                for (GeoLine l : c.getLines()) {
                    lines.add(Arrays.asList(l.x, l.y, l.z));
                }
                f.put("lines", lines);
                break;
            }
            default:
                break;
        }
        return f;
    }

    /**
     * The fields of an arc or sector (see golden/SCHEMA.md): its value (arc
     * length or area) and the type of its conic; for a part of a circle or
     * an ellipse also its orientation, its start and end parameter, its arc
     * length and area and the parts of the conic; for a part that
     * degenerated to a segment or two rays its orientation. GeoGebra keeps
     * the other numbers of a degenerate part from earlier updates, so they
     * are not recorded.
     */
    private static Map<String, Object> arcFields(GeoConicPart p) {
        Map<String, Object> f = new LinkedHashMap<>();
        f.put("value", p.getValue());
        String orientation = p.positiveOrientation() ? "positive" : "negative";
        int t = p.getType();
        switch (t) {
            case GeoConicNDConstants.CONIC_CIRCLE:
            case GeoConicNDConstants.CONIC_ELLIPSE: {
                f.put("conic_type", t == GeoConicNDConstants.CONIC_CIRCLE ? "circle" : "ellipse");
                f.put("orientation", orientation);
                f.put("start", p.getParameterStart());
                f.put("end", p.getParameterEnd());
                f.put("arc_length", p.getArcLength());
                f.put("area", p.getArea());
                Coords mid = p.getMidpoint2D();
                f.put("midpoint", pair(mid.getX(), mid.getY()));
                f.put("half_axes", pair(p.getHalfAxis(0), p.getHalfAxis(1)));
                f.put("eigenvectors", Arrays.asList(
                        pair(p.getEigenvec(0).getX(), p.getEigenvec(0).getY()),
                        pair(p.getEigenvec(1).getX(), p.getEigenvec(1).getY())));
                break;
            }
            case GeoConicNDConstants.CONIC_PARALLEL_LINES:
                f.put("conic_type", "parallel_lines");
                f.put("orientation", orientation);
                break;
            case GeoConicNDConstants.CONIC_SINGLE_POINT:
                f.put("conic_type", "single_point");
                break;
            default:
                throw new IllegalStateException("part of a conic of unsupported type " + t);
        }
        return f;
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
