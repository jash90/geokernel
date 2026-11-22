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
import java.util.TreeMap;

/**
 * Reads case inputs (id, group, script, steps), replays each case in a fresh
 * headless GeoGebra and writes golden files per `golden/SCHEMA.md`.
 *
 * <p>Usage: {@code GoldenGenerator <cases.json> <golden dir> <upstream commit>}.
 * A case id has the form {@code <group>-<topic>-<number>}; all cases of one
 * topic go to {@code <golden dir>/<group>/<topic>.json}.
 * A case may set {@code track_boundary_growth} to record indexed outputs
 * from line or boundary crossings when the reference first labels them.
 * {@code track_root_growth} records newly labelled roots of an interval command.
 */
public final class GoldenGenerator {

    private GoldenGenerator() {
    }

    public static void main(String[] args) throws IOException {
        if (args.length != 3) {
            System.err.println("usage: GoldenGenerator <cases.json> <golden dir> <upstream commit>");
            System.exit(2);
        }
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
        for (Object c : cases) {
            Map<?, ?> input = (Map<?, ?>) c;
            String id = (String) input.get("id");
            String group = (String) input.get("group");
            if (!ids.add(id)) {
                throw new IllegalArgumentException("duplicate case id " + id);
            }
            String prefix = group + "-";
            if (!id.startsWith(prefix) || !id.matches("[a-z]+-[a-z0-9]+-\\d{3,}")) {
                throw new IllegalArgumentException("bad case id " + id);
            }
            String topic = id.substring(prefix.length(), id.lastIndexOf('-'));
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
            file.put("schema", 2L);
            file.put("generator", "ggb-harness");
            file.put("upstream_commit", commit);
            file.put("cases", e.getValue());
            Path out = goldenDir.resolve(e.getKey());
            Files.createDirectories(out.getParent());
            Files.writeString(out, Json.write(file), StandardCharsets.UTF_8);
        }
        System.out.printf("%d cases, %d files, %d snapshots, %d undefined objects recorded%n",
                cases.size(), byFile.size(), stats.snapshots, stats.undefined);
        System.out.println("continuous intersections: " + new Session().kernel().isContinuous());
    }

    private static Map<String, Object> runCase(Map<?, ?> input, Stats stats) {
        boolean boundaryGrowth = Boolean.TRUE.equals(input.get("track_boundary_growth"));
        boolean pointListGrowth = Boolean.TRUE.equals(input.get("track_point_list_growth"));
        boolean rootGrowth = Boolean.TRUE.equals(input.get("track_root_growth"));
        boolean sampleCurves = Boolean.TRUE.equals(input.get("sample_curves"));
        Session session = new Session(boundaryGrowth, pointListGrowth, rootGrowth);
        List<?> script = (List<?>) input.get("script");
        List<?> steps = (List<?>) input.get("steps");
        for (Object line : script) {
            session.run((String) line);
        }
        if (steps.isEmpty() || !"initial".equals(((Map<?, ?>) steps.get(0)).get("action"))) {
            throw new IllegalArgumentException("the first step must be 'initial'");
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
                    session.move(label, x, y);
                    outStep.put("label", label);
                    outStep.put("x", x);
                    outStep.put("y", y);
                    break;
                }
                case "set_number": {
                    String label = (String) step.get("label");
                    double value = number(step.get("value"));
                    session.setNumber(label, value);
                    outStep.put("label", label);
                    outStep.put("value", value);
                    break;
                }
                case "reject": {
                    String line = (String) step.get("line");
                    session.reject(line);
                    outStep.put("line", line);
                    break;
                }
                default:
                    throw new IllegalArgumentException("unknown action " + action);
            }
            if (outStep.size() != step.size()) {
                throw new IllegalArgumentException("unexpected fields in step " + step);
            }
            Map<String, Object> snap = session.snapshot(sampleCurves);
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
        golden.put("script", new ArrayList<>(script));
        golden.put("steps", outSteps);
        golden.put("snapshots", snapshots);
        if (input.size() != (4 + (boundaryGrowth ? 1 : 0) + (pointListGrowth ? 1 : 0)
                + (rootGrowth ? 1 : 0) + (sampleCurves ? 1 : 0))) {
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
