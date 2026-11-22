package geokernel.harness;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import org.geogebra.common.AppCommonFactory;
import org.geogebra.common.factories.UtilFactory;
import org.geogebra.common.factories.UtilFactoryCommon;
import org.geogebra.common.io.FactoryProviderCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.kernel.StringTemplate;
import org.geogebra.common.kernel.commands.AlgebraProcessor;
import org.geogebra.common.kernel.commands.Commands;
import org.geogebra.common.kernel.commands.EvalInfo;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoConicPart;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.implicit.GeoImplicitCurve;
import org.geogebra.common.geogebra3D.kernel3D.geos.GeoCoordSys1D;
import org.geogebra.common.geogebra3D.kernel3D.geos.GeoLine3D;
import org.geogebra.common.geogebra3D.kernel3D.geos.GeoPoint3D;
import org.geogebra.common.geogebra3D.kernel3D.geos.GeoSegment3D;
import org.geogebra.common.geogebra3D.kernel3D.geos.GeoVector3D;
import org.geogebra.common.kernel.kernelND.GeoElementND;
import org.geogebra.common.kernel.matrix.Coords;
import org.geogebra.common.main.App;
import org.geogebra.common.main.PreviewFeature;
import org.geogebra.common.main.error.ErrorHandler;
import org.geogebra.common.util.AsyncOperation;

import com.himamis.retex.renderer.share.platform.FactoryProvider;

/**
 * Replays the test methods of `golden/t-pairs/scripts.jsonl` in GeoGebra and
 * writes what GeoGebra actually returns for every step to
 * `golden/t-pairs/verified.jsonl`.
 *
 * <p>Usage: {@code TPairs <scripts.jsonl> <verified.jsonl> <upstream commit> [type]}.
 * A type argument refreshes only steps that now record an object of that type
 * (including list elements), preserving the other records at the same commit.
 *
 * <p>Each method runs in a fresh app set up the way its test class sets
 * it up (see {@link Profile}); its `t`, `tRound` and `add` steps run in
 * source order through {@code processAlgebraCommandNoExceptionHandling},
 * the call the upstream helper makes, and `seed` and `cas` steps change
 * the random seed and the CAS switch. Statements the extractor could not
 * replay (`other`) are skipped and recorded as an incomplete context.
 * Every output is recorded with its label, class, the string of the
 * template the upstream helper compares, and the golden-format object of
 * {@code golden/SCHEMA.md} (null with a reason when the schema has no type
 * for it).
 */
public final class TPairs {

    private static final String KERNEL_TESTS = "common/kernel/";

    private TPairs() {
    }

    /** How a test class creates its app and compares a result. */
    enum Profile {
        /** {@code CommandTestSetup}: 3D app, seed 42, XML template. */
        COMMANDS,
        /** {@code ArithmeticTest}: 3D app with the CAS on, XML template. */
        ARITHMETIC,
        /** {@code BaseUnitTest} with a 3D app, XML template. */
        BASE_3D,
        /** {@code BaseUnitTest} with the 3D config set, XML template. */
        BASE_3D_CONFIG,
        /** {@code BaseUnitTest} with the editor factory, 3D app, XML template. */
        INPUT_BOX,
        /** {@code BaseUnitTest}: 2D app, XML template. */
        BASE_2D,
        /** A 2D app whose helper compares the test template. */
        TEST_TEMPLATE_2D,
        /** {@code SymbolicArithmeticTest}: symbolic evaluation, algebra template. */
        SYMBOLIC,
        /** {@code FractionTest}: as {@link #SYMBOLIC} with ten print decimals. */
        SYMBOLIC_10;
    }

    /** The profile of every file that holds pairs; a file not listed stops the run. */
    private static final Map<String, Profile> PROFILES = new TreeMap<>();

    static {
        PROFILES.put("commands/CommandsTest.java", Profile.COMMANDS);
        PROFILES.put("commands/CmdImplicitSurfaceTest.java", Profile.COMMANDS);
        PROFILES.put("commands/CommandsUsingMockedCasTest.java", Profile.COMMANDS);
        PROFILES.put("arithmetic/ArithmeticTest.java", Profile.ARITHMETIC);
        PROFILES.put("commands/RedefineTest.java", Profile.BASE_3D);
        PROFILES.put("geos/GeoInputBoxLinkedGeoTest.java", Profile.INPUT_BOX);
        PROFILES.put("kernelND/GeoSurfaceCartesian3DTest.java", Profile.BASE_3D_CONFIG);
        PROFILES.put("commands/CommandsIn2DAppTest.java", Profile.TEST_TEMPLATE_2D);
        PROFILES.put("cas/AlgoDerivativeTest.java", Profile.TEST_TEMPLATE_2D);
        PROFILES.put("arithmetic/FractionTest.java", Profile.SYMBOLIC_10);
        PROFILES.put("arithmetic/SurdTest.java", Profile.SYMBOLIC);
        PROFILES.put("algos/AlgoExtremumMultiTest.java", Profile.BASE_2D);
        PROFILES.put("algos/AlgoNumeratorDenominatorFunTest.java", Profile.BASE_2D);
        PROFILES.put("commands/CmdCopyFreeObjectTest.java", Profile.BASE_2D);
        PROFILES.put("commands/FormulaTextTest.java", Profile.BASE_2D);
        PROFILES.put("commands/TranslateTest.java", Profile.BASE_2D);
        PROFILES.put("geos/GeoFunctionTest.java", Profile.BASE_2D);
        PROFILES.put("implicit/GeoImplicitCurveTest.java", Profile.BASE_2D);
    }

    /** Files whose expected strings come from a mocked CAS, not from GeoGebra. */
    private static final String MOCKED_CAS = "commands/CommandsUsingMockedCasTest.java";
    /** The file whose class enables GeoGebra's preview features. */
    private static final String PREVIEW = "commands/CmdImplicitSurfaceTest.java";

    public static void main(String[] args) throws IOException {
        if (args.length != 3 && args.length != 4) {
            System.err.println("usage: TPairs <scripts.jsonl> <verified.jsonl> <upstream commit> [type]");
            System.exit(2);
        }
        String commit = args[2];
        if (!commit.matches("[0-9a-f]{40}")) {
            throw new IllegalArgumentException("not a full commit hash: " + commit);
        }
        Path output = Path.of(args[1]);
        String type = args.length == 4 ? args[3] : null;
        Map<Object, String> previous = type == null ? Map.of() : previousRecords(output, commit);
        UtilFactory.setPrototypeIfNull(new UtilFactoryCommon());
        List<String> out = new ArrayList<>();
        Map<String, Object> header = new LinkedHashMap<>();
        header.put("generator", "ggb-harness");
        header.put("upstream_commit", commit);
        header.put("source", "golden/t-pairs/scripts.jsonl");
        out.add(Json.writeLine(header));
        int steps = 0;
        int differ = 0;
        for (String line : Files.readAllLines(Path.of(args[0]), StandardCharsets.UTF_8)) {
            if (line.isBlank()) {
                continue;
            }
            Map<?, ?> method = (Map<?, ?>) Json.parse(line);
            for (Map<String, Object> record : runMethod(method)) {
                if (type == null || includesType(record.get("outputs"), type)) {
                    out.add(Json.writeLine(record));
                } else {
                    String existing = previous.get(record.get("id"));
                    if (existing == null) {
                        throw new IllegalArgumentException("missing prior record " + record.get("id"));
                    }
                    out.add(existing);
                }
                steps++;
                if (Boolean.FALSE.equals(record.get("matches_expected"))) {
                    differ++;
                }
            }
        }
        Files.write(output, out, StandardCharsets.UTF_8);
        System.out.printf("%d steps recorded, %d differ from the upstream expected strings%n",
                steps, differ);
    }

    private static Map<Object, String> previousRecords(Path output, String commit) throws IOException {
        List<String> lines = Files.readAllLines(output, StandardCharsets.UTF_8);
        if (lines.isEmpty() || !commit.equals(((Map<?, ?>) Json.parse(lines.get(0))).get("upstream_commit"))) {
            throw new IllegalArgumentException("the prior records need the same upstream commit");
        }
        Map<Object, String> records = new LinkedHashMap<>();
        for (String line : lines.subList(1, lines.size())) {
            records.put(((Map<?, ?>) Json.parse(line)).get("id"), line);
        }
        return records;
    }

    private static boolean includesType(Object value, String type) {
        if (value instanceof List<?> items) {
            return items.stream().anyMatch(item -> includesType(item, type));
        }
        if (value instanceof Map<?, ?> object) {
            return type.equals(object.get("type"))
                    || includesType(object.get("object"), type)
                    || includesType(object.get("elements"), type);
        }
        return false;
    }

    private static List<Map<String, Object>> runMethod(Map<?, ?> method) {
        String file = (String) method.get("file");
        String rel = file.substring(file.indexOf(KERNEL_TESTS) + KERNEL_TESTS.length());
        Profile profile = PROFILES.get(rel);
        if (profile == null) {
            throw new IllegalStateException("no profile for " + rel);
        }
        PreviewFeature.setPreviewFeaturesEnabled(PREVIEW.equals(rel));
        AppCommon app = createApp(profile);
        AlgebraProcessor processor = app.getKernel().getAlgebraProcessor();
        StringTemplate template = template(profile);
        boolean complete = Boolean.TRUE.equals(method.get("complete"));
        boolean contextComplete = complete;
        List<Map<String, Object>> records = new ArrayList<>();
        for (Object s : (List<?>) method.get("steps")) {
            Map<?, ?> step = (Map<?, ?>) s;
            String call = (String) step.get("call");
            switch (call) {
                case "other":
                    contextComplete = false;
                    continue;
                case "seed":
                    app.setRandomSeed(((Long) step.get("value")).intValue());
                    continue;
                case "cas":
                    app.getSettings().getCasSettings().setEnabled((Boolean) step.get("value"));
                    continue;
                case "t":
                case "tRound":
                case "add":
                    break;
                default:
                    throw new IllegalStateException("unknown step " + call);
            }
            StringTemplate tpl = "tRound".equals(call) ? StringTemplate.editTemplate : template;
            String input = (String) step.get("input");
            Map<String, Object> record = new LinkedHashMap<>();
            record.put("id", rel + ":" + step.get("line"));
            record.put("test", method.get("test"));
            record.put("call", call);
            record.put("input", input);
            record.put("template", templateName(call, profile));
            record.put("context_complete", contextComplete);
            List<?> expected = (List<?>) step.get("expected");
            if (expected != null) {
                record.put("expected", expected);
            }
            GeoElementND[] result = null;
            String error = null;
            try {
                result = run(processor, input, profile);
            } catch (Throwable e) {
                error = e.getClass().getSimpleName() + ": " + e.getMessage();
            }
            List<Object> outputs = new ArrayList<>();
            List<String> strings = new ArrayList<>();
            if (result != null) {
                for (GeoElementND nd : result) {
                    GeoElement geo = nd.toGeoElement();
                    String string = geo.toValueString(tpl);
                    strings.add(string);
                    outputs.add(output(geo, string));
                }
            }
            if (error != null) {
                record.put("error", error);
            } else if (result == null) {
                record.put("error", "no result");
            }
            record.put("outputs", outputs);
            if (expected != null) {
                record.put("matches_expected", error == null && result != null
                        && strings.equals(expected));
            }
            if (MOCKED_CAS.equals(rel)) {
                record.put("note", "the upstream test expects values from a mocked CAS");
            }
            records.add(record);
        }
        PreviewFeature.setPreviewFeaturesEnabled(false);
        return records;
    }

    private static AppCommon createApp(Profile profile) {
        AppCommon app;
        switch (profile) {
            case COMMANDS:
                app = AppCommonFactory.create3D();
                app.setRandomSeed(42);
                app.setActiveView(App.VIEW_EUCLIDIAN);
                GeoImplicitCurve.setFastDrawThreshold(10000);
                return app;
            case ARITHMETIC:
                app = AppCommonFactory.create3D();
                app.getSettings().getCasSettings().setEnabled(true);
                app.getKernel().clearConstruction(true);
                break;
            case BASE_3D:
                app = AppCommonFactory.create3D();
                app.getKernel().setLoadingMode(false);
                break;
            case BASE_3D_CONFIG:
                app = AppCommonFactory.create3D();
                app.set3dConfig();
                break;
            case INPUT_BOX:
                FactoryProvider.setInstance(new FactoryProviderCommon());
                app = AppCommonFactory.create3D();
                break;
            case TEST_TEMPLATE_2D:
                app = AppCommonFactory.create();
                app.setLanguage("en");
                break;
            case SYMBOLIC_10:
                app = AppCommonFactory.create();
                app.getKernel().setPrintDecimals(10);
                break;
            case SYMBOLIC:
            case BASE_2D:
                app = AppCommonFactory.create();
                break;
            default:
                throw new IllegalStateException(profile.name());
        }
        // Only random commands depend on it; the upstream classes leave the
        // seed unset, which would make two runs differ.
        app.setRandomSeed(42);
        return app;
    }

    private static StringTemplate template(Profile profile) {
        switch (profile) {
            case TEST_TEMPLATE_2D:
                return StringTemplate.testTemplate;
            case SYMBOLIC:
            case SYMBOLIC_10:
                return StringTemplate.algebraTemplate;
            default:
                return StringTemplate.xmlTemplate;
        }
    }

    private static String templateName(String call, Profile profile) {
        if ("tRound".equals(call)) {
            return "edit";
        }
        switch (profile) {
            case TEST_TEMPLATE_2D:
                return "test";
            case SYMBOLIC:
            case SYMBOLIC_10:
                return "algebra";
            default:
                return "xml";
        }
    }

    private static GeoElementND[] run(AlgebraProcessor processor, String input, Profile profile) {
        ErrorHandler handler = new RecordingErrorHandler(input);
        if (profile == Profile.SYMBOLIC || profile == Profile.SYMBOLIC_10) {
            return processor.processAlgebraCommandNoExceptionHandling(
                    input, false, handler, new EvalInfo(true).withSymbolic(true), null);
        }
        return processor.processAlgebraCommandNoExceptionHandling(input, false, handler, false, null);
    }

    private static Map<String, Object> output(GeoElement geo, String string) {
        Map<String, Object> o = new LinkedHashMap<>();
        o.put("label", geo.isLabelSet() ? geo.getLabelSimple() : null);
        o.put("class", geo.getClass().getSimpleName());
        o.put("string", string);
        try {
            Map<String, Object> planar = planar(geo);
            o.put("object", planar != null ? planar : Session.describe(geo, recordedType(geo)));
        } catch (RuntimeException e) {
            o.put("object", null);
            o.put("unsupported", e.getMessage());
        }
        return o;
    }

    /**
     * The 3D app makes some planar results as 3D objects (`GeoPoint3D`,
     * `GeoSegment3D`, `GeoLine3D`, `GeoVector3D`). One whose coordinates
     * all lie in the plane z = 0 is recorded as the 2D object it is: a point
     * or vector by its x and y, a segment by its end points, a line by
     * the coefficients GeoGebra's 2D line through its start point `P` and
     * `P + d` has (`a = -d.y`, `b = d.x`, `c = P.x d.y - P.y d.x`, the cross
     * product of the two points). Null for any other object; an error for
     * one that leaves the plane.
     */
    private static Map<String, Object> planar(GeoElement geo) {
        Map<String, Object> o = new LinkedHashMap<>();
        if (geo instanceof GeoPoint3D) {
            GeoPoint3D p = (GeoPoint3D) geo;
            o.put("type", "point");
            Coords c = p.getInhomCoordsInD3();
            if (!p.isDefined() || p.isInfinite() || !finite(c, 3)) {
                o.put("defined", false);
                return o;
            }
            requirePlanar(c.getZ());
            o.put("defined", true);
            o.put("x", c.getX());
            o.put("y", c.getY());
            return o;
        }
        if (geo instanceof GeoVector3D) {
            GeoVector3D v = (GeoVector3D) geo;
            o.put("type", "vector");
            Coords c = v.getCoordsInD3();
            if (!v.isDefined() || !finite(c, 3)) {
                o.put("defined", false);
                return o;
            }
            requirePlanar(c.getZ());
            o.put("defined", true);
            o.put("x", c.getX());
            o.put("y", c.getY());
            return o;
        }
        boolean segment = geo instanceof GeoSegment3D;
        boolean line = geo.getClass() == GeoLine3D.class;
        if (!segment && !line) {
            return null;
        }
        GeoCoordSys1D l = (GeoCoordSys1D) geo;
        o.put("type", segment ? "segment" : "line");
        Coords p = l.getStartInhomCoords();
        Coords d = l.getDirectionInD3();
        if (!geo.isDefined() || !finite(p, 3) || !finite(d, 3)) {
            o.put("defined", false);
            return o;
        }
        requirePlanar(p.getZ());
        requirePlanar(d.getZ());
        o.put("defined", true);
        if (segment) {
            o.put("x1", p.getX());
            o.put("y1", p.getY());
            o.put("x2", p.getX() + d.getX());
            o.put("y2", p.getY() + d.getY());
        } else {
            o.put("a", -d.getY());
            o.put("b", d.getX());
            o.put("c", p.getX() * d.getY() - p.getY() * d.getX());
        }
        return o;
    }

    private static boolean finite(Coords c, int n) {
        for (int i = 1; i <= n; i++) {
            if (!Double.isFinite(c.get(i))) {
                return false;
            }
        }
        return true;
    }

    private static void requirePlanar(double z) {
        if (z != 0) {
            throw new IllegalStateException("3D object off the plane z = 0");
        }
    }

    /**
     * The schema type of an output. A conic is a `circle` when it is a
     * circle (or a single point) made by a command other than the general
     * conic commands; an independent conic (an equation) and every other
     * conic is a `conic`. Everything else is typed as in golden files.
     */
    static String recordedType(GeoElement geo) {
        if (geo instanceof GeoConic && !(geo instanceof GeoConicPart)) {
            int t = ((GeoConic) geo).getType();
            boolean circle = t == GeoConic.CONIC_CIRCLE || t == GeoConic.CONIC_SINGLE_POINT;
            if (!circle || geo.getParentAlgorithm() == null) {
                return "conic";
            }
            Object command = geo.getParentAlgorithm().getClassName();
            boolean general = command == Commands.Conic || command == Commands.Ellipse
                    || command == Commands.Hyperbola || command == Commands.Parabola;
            return general ? "conic" : "circle";
        }
        return Session.typeOf(geo);
    }

    /** Turns every GeoGebra error report into an exception with its message. */
    private static final class RecordingErrorHandler implements ErrorHandler {
        private final String input;

        RecordingErrorHandler(String input) {
            this.input = input;
        }

        @Override
        public void showError(String msg) {
            throw new IllegalStateException(msg);
        }

        @Override
        public void showCommandError(String command, String message) {
            throw new IllegalStateException(message);
        }

        @Override
        public String getCurrentCommand() {
            return input;
        }

        @Override
        public boolean onUndefinedVariables(String string, AsyncOperation<String[]> callback) {
            throw new IllegalStateException("undefined variables: " + string);
        }

        @Override
        public void resetError() {
            // nothing to reset
        }
    }
}
