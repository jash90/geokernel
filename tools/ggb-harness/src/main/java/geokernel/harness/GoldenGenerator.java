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

import org.geogebra.common.main.MyParseError;
import java.util.Set;
import java.util.TreeMap;

/**
 * Reads case inputs (id, group, script, steps), replays each case in a fresh
 * headless GeoGebra and writes golden files per `golden/SCHEMA.md`.
 *
 * <p>Usage: {@code GoldenGenerator <cases.json> <golden dir> <upstream commit> [topics]}.
 * {@code topics} is a comma-separated list of topics to record, or, with a
 * leading {@code !}, of topics to leave out; without it every case is recorded.
 * A case id has the form {@code <group>-<topic>-<number>}; all cases of one
 * topic go to {@code <golden dir>/<group>/<topic>.json}.
 * A case may set {@code track_boundary_growth} to record indexed outputs
 * from line or boundary crossings and inequality vertices when they first acquire labels.
 * {@code track_root_growth} records newly labelled roots of an interval command.
 * {@code file_loading} exercises the legacy {@code Function} interval command
 * and emits the same script form as the saved-construction importer.
 */
public final class GoldenGenerator {

    private GoldenGenerator() {
    }

    public static void main(String[] args) throws IOException {
        if (args.length < 3 || args.length > 4) {
            System.err.println("usage: GoldenGenerator <cases.json> <golden dir> <upstream commit> [topics or !topics]");
            System.exit(2);
        }
        Session.recordInfinity = true;
        Path casesFile = Path.of(args[0]);
        Path goldenDir = Path.of(args[1]);
        String commit = args[2];
        if (!commit.matches("[0-9a-f]{40}")) {
            throw new IllegalArgumentException("not a full commit hash: " + commit);
        }

        Object parsed = Json.parse(Files.readString(casesFile, StandardCharsets.UTF_8));
        List<?> cases = (List<?>) ((Map<?, ?>) parsed).get("cases");
        Map<String, List<Object>> byFile = new TreeMap<>();
        Set<String> ids = new HashSet<>();
        Stats stats = new Stats();
        String selector = args.length == 4 ? args[3] : "";
        boolean excluded = selector.startsWith("!");
        Set<String> selected = new HashSet<>();
        for (String topic : (excluded ? selector.substring(1) : selector).split(",")) {
            if (!topic.isEmpty()) {
                selected.add(topic);
            }
        }
        int count = 0;
        for (Object c : cases) {
            Map<?, ?> input = (Map<?, ?>) c;
            String id = (String) input.get("id");
            String group = (String) input.get("group");
            if (("cas".equals(group) || "cascell".equals(group) || "curvepairs".equals(group)) && !GiacOracle.enabled()) {
                throw new IllegalStateException("CAS fixtures require the native Giac oracle");
            }
            if (!ids.add(id)) {
                throw new IllegalArgumentException("duplicate case id " + id);
            }
            String prefix = group + "-";
            if (!id.startsWith(prefix) || !id.matches("[a-z]+-[a-z0-9]+-\\d{3,}")) {
                throw new IllegalArgumentException("bad case id " + id);
            }
            String topic = id.substring(prefix.length(), id.lastIndexOf('-'));
            if (!selector.isEmpty() && (excluded ? selected.contains(topic) : !selected.contains(topic))) {
                continue;
            }
            count++;
            GiacOracle.currentCase = id;
            Map<String, Object> golden;
            try {
                golden = runCase(input, stats);
            } catch (RuntimeException e) {
                throw new IllegalStateException("case " + id + " failed: " + e.getMessage(), e);
            }
            byFile.computeIfAbsent(group + "/" + topic + ".json", k -> new ArrayList<>())
                    .add(golden);
        }

        for (Map.Entry<String, List<Object>> e : byFile.entrySet()) {
            Map<String, Object> file = new LinkedHashMap<>();
            file.put("schema", 4L);
            file.put("generator", "ggb-harness");
            file.put("upstream_commit", commit);
            file.put("cases", e.getValue());
            Path out = goldenDir.resolve(e.getKey());
            Files.createDirectories(out.getParent());
            Files.writeString(out, Json.write(file), StandardCharsets.UTF_8);
        }
        System.out.printf("%d cases, %d files, %d snapshots, %d undefined objects recorded%n",
                count, byFile.size(), stats.snapshots, stats.undefined);
        System.out.println("continuous intersections: " + new Session().kernel().isContinuous());
    }

    private static Map<String, Object> runCase(Map<?, ?> input, Stats stats) {
        boolean boundaryGrowth = Boolean.TRUE.equals(input.get("track_boundary_growth"));
        boolean pointListGrowth = Boolean.TRUE.equals(input.get("track_point_list_growth"));
        boolean rootGrowth = Boolean.TRUE.equals(input.get("track_root_growth"));
        boolean sampleCurves = Boolean.TRUE.equals(input.get("sample_curves"));
        boolean fileLoading = Boolean.TRUE.equals(input.get("file_loading"));
        Session session = new Session(boundaryGrowth, pointListGrowth, rootGrowth);
        List<?> view = (List<?>) input.get("view");
        if (view != null) {
            if (view.size() != 6) {
                throw new IllegalArgumentException("view needs four bounds and two scales");
            }
            double[] bounds = new double[6];
            for (int i = 0; i < bounds.length; i++) {
                bounds[i] = number(view.get(i));
            }
            session.setView(bounds);
        }
        List<?> script = (List<?>) input.get("script");
        List<?> steps = (List<?>) input.get("steps");
        List<?> observe = (List<?>) input.get("observe");
        boolean undo = steps.stream().anyMatch(step -> "undo".equals(((Map<?, ?>) step).get("action")));
        if (undo) {
            session.kernel().setUndoActive(true);
        }
        session.kernel().getConstruction().setFileLoading(fileLoading);
        try {
            for (Object inputLine : script) {
                String line = (String) inputLine;
                if (observe != null && line.contains(" = IntersectPath(")) {
                    session.sampleClip(line, observe);
                } else {
                    session.run(line);
                }
            }
        } finally {
            session.kernel().getConstruction().setFileLoading(false);
        }
        if (steps.isEmpty() || !"initial".equals(((Map<?, ?>) steps.get(0)).get("action"))) {
            throw new IllegalArgumentException("the first step must be 'initial'");
        }
        if (undo) {
            session.kernel().initUndoInfo();
        }
        List<Object> outSteps = new ArrayList<>();
        List<Object> snapshots = new ArrayList<>();
        for (int i = 0; i < steps.size(); i++) {
            Map<?, ?> step = (Map<?, ?>) steps.get(i);
            Map<String, Object> outStep = new LinkedHashMap<>();
            String action = (String) step.get("action");
            outStep.put("action", action);
            switch (action) {
                case "initial":
                    if (i != 0) {
                        throw new IllegalArgumentException("'initial' must be the first step only");
                    }
                    break;
                case "move": {
                    String label = (String) step.get("label");
                    double x = number(step.get("x"));
                    double y = number(step.get("y"));
                    if (Boolean.TRUE.equals(step.get("reference_parse_error"))) {
                        boolean failed = false;
                        try {
                            session.move(label, x, y);
                        } catch (MyParseError error) {
                            failed = true;
                        }
                        outStep.put("reference_parse_error", failed);
                    } else {
                        session.move(label, x, y);
                    }
                    outStep.put("label", label);
                    outStep.put("x", x);
                    outStep.put("y", y);
                    break;
                }
                case "set_number": {
                    String label = (String) step.get("label");
                    outStep.put("label", label);
                    Object infinity = step.get("infinity");
                    double value;
                    if (infinity == null) {
                        value = number(step.get("value"));
                        outStep.put("value", value);
                    } else if (Long.valueOf(1).equals(infinity) || Long.valueOf(-1).equals(infinity)) {
                        value = number(infinity) * Double.POSITIVE_INFINITY;
                        outStep.put("infinity", infinity);
                    } else {
                        throw new IllegalArgumentException("infinity must be 1 or -1");
                    }
                    session.setNumber(label, value);
                    break;
                }
                case "delete": {
                    String label = (String) step.get("label");
                    session.delete(label);
                    outStep.put("label", label);
                    break;
                }
                case "add":
                case "append": {
                    String line = (String) step.get("line");
                    session.run(line);
                    outStep.put("line", line);
                    break;
                }
                case "anchor_text": {
                    String label = (String) step.get("label");
                    String point = (String) step.get("point");
                    session.anchorText(label, point);
                    outStep.put("label", label);
                    outStep.put("point", point);
                    break;
                }
                case "undo":
                case "redo":
                case "reload":
                    session.restoreStep(action);
                    break;
                case "redefine": {
                    String line = (String) step.get("line");
                    session.redefine(line);
                    outStep.put("line", line);
                    break;
                }
                case "reload_ggb":
                    session.reloadGgb();
                    break;
                case "slider_animation": {
                    String label = (String) step.get("label");
                    outStep.put("label", label);
                    outStep.put("options", session.sliderAnimation(label));
                    break;
                }
                case "set_slider_bounds": {
                    String label = (String) step.get("label");
                    double min = number(step.get("min"));
                    double max = number(step.get("max"));
                    session.setSliderBounds(label, min, max);
                    outStep.put("label", label);
                    outStep.put("min", min);
                    outStep.put("max", max);
                    break;
                }
                case "animate": {
                    int frames = (int) number(step.get("frames"));
                    if (frames != number(step.get("frames")) || frames < 1 || frames > 10000) {
                        throw new IllegalArgumentException("bad frame count in " + step);
                    }
                    session.animate(frames);
                    outStep.put("frames", (double) frames);
                    break;
                }
                case "reject": {
                    String line = (String) step.get("line");
                    session.reject(line);
                    outStep.put("line", line);
                    // The kernel's own failure for the line, checked by the
                    // Rust replay; GeoGebra's message is not recorded.
                    if (step.containsKey("error")) {
                        outStep.put("error", step.get("error"));
                    }
                    break;
                }
                default:
                    throw new IllegalArgumentException("unknown action " + action);
            }
            if (outStep.size() != step.size() + ("slider_animation".equals(action) ? 1 : 0)) {
                throw new IllegalArgumentException("unexpected fields in step " + step);
            }
            if (undo && i > 0 && !Set.of("undo", "redo", "reload").contains(action)) {
                session.kernel().storeUndoInfo();
            }
            Map<String, Object> snap = session.snapshot(sampleCurves, observe);
            if (observe != null && !snap.keySet().equals(new HashSet<>(observe))) {
                throw new IllegalStateException("observed labels " + observe + ", found " + snap.keySet());
            }
            for (Object o : snap.values()) {
                if (Boolean.FALSE.equals(((Map<?, ?>) o).get("defined"))) {
                    stats.undefined++;
                }
            }
            stats.snapshots++;
            outSteps.add(outStep);
            snapshots.add(snap);
        }

        Map<String, Object> golden = new LinkedHashMap<>();
        golden.put("id", input.get("id"));
        golden.put("group", input.get("group"));
        golden.put("script", fileLoading ? script.stream()
                .map(line -> ((String) line).replace(" = Function(", " = $functionInterval("))
                .toList() : new ArrayList<>(script));
        if (view != null) {
            golden.put("view", view);
        }
        if (observe != null) {
            golden.put("observe", observe);
        }
        golden.put("steps", outSteps);
        golden.put("snapshots", snapshots);
        String row = (String) input.get("cascell");
        if ("cascell".equals(input.get("group")) != (row != null)
                || row != null && (row.isBlank() || steps.size() != 1)) {
            throw new IllegalArgumentException("a cascell case needs one input and only the initial step");
        }
        if (row != null) {
            golden.put("cascell", session.casCell(row));
        }
        if (input.size() != (4 + (boundaryGrowth ? 1 : 0) + (pointListGrowth ? 1 : 0)
                + (rootGrowth ? 1 : 0) + (sampleCurves ? 1 : 0) + (view != null ? 1 : 0)
                + (fileLoading ? 1 : 0) + (observe != null ? 1 : 0) + (row != null ? 1 : 0))) {
            throw new IllegalArgumentException(
                    "unexpected fields in the case input");
        }
        return golden;
    }

    private static double number(Object value) {
        if (value instanceof Long) {
            return ((Long) value).doubleValue();
        }
        if (value instanceof Double) {
            return (Double) value;
        }
        throw new IllegalArgumentException("not a number: " + value);
    }

    private static final class Stats {
        int snapshots;
        int undefined;
    }
}
