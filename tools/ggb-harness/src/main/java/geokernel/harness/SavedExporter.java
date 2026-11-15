package geokernel.harness;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoLine;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoSegment;
import org.geogebra.common.kernel.kernelND.GeoElementND;
import org.geogebra.common.main.error.ErrorHandler;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.common.util.AsyncOperation;

/**
 * Saves constructions after their free points were moved, together with what
 * GeoGebra computes at that moment, to prove how a saved file relates to the
 * state it was saved from.
 *
 * <p>Usage: {@code SavedExporter <saved.json> <out dir>}. Every case has an
 * id, optionally an {@code angle_unit} ({@code degree} or {@code radiant}), raw
 * GeoGebra input lines, and {@code move} steps. The lines are given to the
 * algebra processor as typed (so a command that makes several objects, such as
 * {@code P = Intersect(f, k)}, gives all of them), the moves are applied, and
 * then, with nothing else done in between, the case is written as
 * {@code <out dir>/<id>.xml} (the text {@code app.getXML()} returns) and
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
        if (args.length != 2) {
            System.err.println("usage: SavedExporter <saved.json> <out dir>");
            System.exit(2);
        }
        Object parsed = Json.parse(Files.readString(Path.of(args[0]), StandardCharsets.UTF_8));
        List<?> cases = (List<?>) ((Map<?, ?>) parsed).get("cases");
        Path outDir = Path.of(args[1]);
        Files.createDirectories(outDir);
        Set<String> produced = new HashSet<>();
        for (Object o : cases) {
            Map<?, ?> input = (Map<?, ?>) o;
            String id = (String) input.get("id");
            try {
                export(input, outDir);
            } catch (RuntimeException e) {
                throw new IllegalStateException("case " + id + " failed: " + e.getMessage(), e);
            }
            produced.add(id + ".xml");
            produced.add(id + ".snapshot");
        }
        try (var files = Files.list(outDir)) {
            for (Path file : (Iterable<Path>) files::iterator) {
                if (!produced.contains(file.getFileName().toString())) {
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
        List<String> script = new ArrayList<>();
        for (Object line : (List<?>) input.get("script")) {
            script.add((String) line);
            GeoElementND[] out = kernel.getAlgebraProcessor()
                    .processAlgebraCommandNoExceptionHandling((String) line, false,
                            new ThrowingErrorHandler((String) line), false, null);
            if (out == null) {
                throw new IllegalStateException("no object from " + line);
            }
            for (GeoElementND nd : out) {
                created.add(nd.toGeoElement());
            }
        }
        for (Object s : (List<?>) input.get("steps")) {
            Map<?, ?> step = (Map<?, ?>) s;
            if (!"move".equals(step.get("action"))) {
                throw new IllegalArgumentException("only move steps are supported: " + step);
            }
            GeoElement geo = kernel.lookupLabel((String) step.get("label"));
            if (!(geo instanceof GeoPoint) || !geo.isIndependent()) {
                throw new IllegalArgumentException(step.get("label") + " is not a free point");
            }
            GeoPoint p = (GeoPoint) geo;
            p.setCoords(number(step.get("x")), number(step.get("y")), 1.0);
            p.updateCascade();
        }
        // The state to save, and what GeoGebra computes in it.
        Map<String, Object> objects = new LinkedHashMap<>();
        for (GeoElement geo : created) {
            if (geo.isLabelSet() && kernel.lookupLabel(geo.getLabelSimple()) == geo) {
                objects.put(geo.getLabelSimple(), describe(geo));
            }
        }
        Files.writeString(outDir.resolve(id + ".xml"), app.getXML(), StandardCharsets.UTF_8);
        Map<String, Object> snapshot = new LinkedHashMap<>();
        snapshot.put("schema", 1L);
        snapshot.put("id", id);
        snapshot.put("script", script);
        snapshot.put("objects", objects);
        Files.writeString(outDir.resolve(id + ".snapshot"), Json.write(snapshot),
                StandardCharsets.UTF_8);
    }

    private static double number(Object value) {
        if (value instanceof Long) {
            return ((Long) value).doubleValue();
        }
        return (Double) value;
    }

    /** The golden-file form of an object (only the types the saved cases use). */
    private static Map<String, Object> describe(GeoElement geo) {
        Map<String, Object> obj = new LinkedHashMap<>();
        Map<String, Object> f = new LinkedHashMap<>();
        boolean ok = geo.isDefined();
        if (geo instanceof GeoPoint) {
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
                throw new IllegalStateException(geo.getLabelSimple() + " is not a circle");
            }
            obj.put("type", "circle");
            if (ok) {
                f.put("cx", c.getMidpoint2D().getX());
                f.put("cy", c.getMidpoint2D().getY());
                f.put("r", c.getHalfAxis(0));
            }
        } else if (geo instanceof GeoNumeric) {
            obj.put("type", "number");
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
