package geokernel.harness;

import java.lang.management.ManagementFactory;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoPoint;

/**
 * Builds in GeoGebra the same chain as {@code bench()} in
 * {@code crates/geo-cli/src/main.rs} and times dragging its free point A.
 *
 * <p>Per step i the chain adds a line through A and the previous point, the
 * circle around A through that point, the perpendicular to the line through
 * A, its intersection with the circle, the midpoint of the previous point and
 * that intersection, the line through A and the midpoint, and that line's
 * second intersection with the circle, which becomes the next previous point.
 * Labels use subscripts because names such as {@code P1} are spreadsheet cells.
 *
 * <p>Usage: {@code ChainTiming [steps]}, default 15000 like the Rust bench.
 * GeoGebra keeps for every object the set of all algorithms that depend on
 * it, so memory grows with the square of the chain length and the default
 * size does not fit in memory; use a few hundred to a thousand steps.
 */
public final class ChainTiming {

    private static final int DRAGS = 200;
    private static final int TIMED_ROUNDS = 3;
    private static final int LOCAL_DRAGS = 100_000;

    private ChainTiming() {
    }

    public static void main(String[] args) {
        int steps = args.length > 0 ? Integer.parseInt(args[0]) : 15_000;
        Session s = new Session();
        // Time the construction kernel only: the headless app's graphics view
        // would otherwise repaint every object after each change.
        s.kernel().setNotifyViewsActive(false);
        List<GeoElement> chain = new ArrayList<>();

        long t0 = System.nanoTime();
        GeoPoint a = (GeoPoint) s.run("A = (0.3, 0.7)");
        chain.add(a);
        chain.add(s.run("B = (10, 2)"));
        String p = "B";
        for (int i = 0; i < steps; i++) {
            String l = "l_{" + i + "}";
            String k = "k_{" + i + "}";
            String n = "n_{" + i + "}";
            String x = "X_{" + i + "}";
            String m = "M_{" + i + "}";
            String g = "g_{" + i + "}";
            String next = "P_{" + i + "}";
            chain.add(s.run(l + " = Line(A, " + p + ")"));
            chain.add(s.run(k + " = Circle(A, " + p + ")"));
            chain.add(s.run(n + " = PerpendicularLine(A, " + l + ")"));
            chain.add(s.run(x + " = Intersect(" + n + ", " + k + ", 1)"));
            chain.add(s.run(m + " = Midpoint(" + p + ", " + x + ")"));
            chain.add(s.run(g + " = Line(A, " + m + ")"));
            chain.add(s.run(next + " = Intersect(" + g + ", " + k + ", 2)"));
            p = next;
        }
        double buildMs = (System.nanoTime() - t0) / 1e6;

        GeoPoint z1 = (GeoPoint) s.run("Z_{1} = (50, 50)");
        s.run("Z_{2} = (52, 51)");
        s.run("Z_{m} = Midpoint(Z_{1}, Z_{2})");
        int objects = chain.size() + 3;
        int dependent = chain.size() - 2;
        requireDefined(chain, "after the build");
        System.out.printf(Locale.ROOT, "continuous intersections: %s%n", s.kernel().isContinuous());
        System.out.printf(Locale.ROOT, "objects: %d (%d depend on A)%n", objects, dependent);
        System.out.printf(Locale.ROOT, "build: %.1f ms%n", buildMs);
        System.gc();
        Runtime rt = Runtime.getRuntime();
        System.out.printf(Locale.ROOT, "heap in use after the build: %.0f MB%n",
                (rt.totalMemory() - rt.freeMemory()) / 1e6);
        System.out.printf(Locale.ROOT, "update set of A: %d algorithms%n",
                a.getAlgoUpdateSet().getSize());

        dragRound(a);
        requireDefined(chain, "after the warm-up");
        double[] perDrag = new double[TIMED_ROUNDS];
        for (int r = 0; r < TIMED_ROUNDS; r++) {
            long start = System.nanoTime();
            dragRound(a);
            perDrag[r] = (System.nanoTime() - start) / 1e6 / DRAGS;
            System.out.printf(Locale.ROOT,
                    "round %d: %.3f ms per drag of A, %.1f ns per dependent object%n",
                    r + 1, perDrag[r], perDrag[r] * 1e6 / dependent);
        }
        requireDefined(chain, "after the timed drags");

        long start = System.nanoTime();
        for (int i = 0; i < LOCAL_DRAGS; i++) {
            z1.setCoords(50.0 + ((i % 1000) + 1) * 0.001, 50.0, 1.0);
            z1.updateCascade();
        }
        double localNs = (System.nanoTime() - start) / (double) LOCAL_DRAGS;
        System.out.printf(Locale.ROOT, "local drag (Z_1): %.0f ns per drag%n", localNs);

        double[] sorted = perDrag.clone();
        Arrays.sort(sorted);
        System.out.printf(Locale.ROOT, "best of %d rounds: %.3f ms per drag%n",
                TIMED_ROUNDS, sorted[0]);
        System.out.printf(Locale.ROOT, "jvm: %s %s (%s), args %s%n",
                System.getProperty("java.vm.vendor"), System.getProperty("java.vm.name"),
                System.getProperty("java.runtime.version"),
                ManagementFactory.getRuntimeMXBean().getInputArguments());
        System.out.printf(Locale.ROOT, "os: %s %s, %d processors%n",
                System.getProperty("os.name"), System.getProperty("os.arch"),
                Runtime.getRuntime().availableProcessors());
    }

    /** Drags A around a small circle, exactly like the Rust bench. */
    private static void dragRound(GeoPoint a) {
        for (int i = 0; i < DRAGS; i++) {
            double t = i * 0.05;
            a.setCoords(0.3 + Math.cos(t), 0.7 + Math.sin(t), 1.0);
            a.updateCascade();
        }
    }

    private static void requireDefined(List<GeoElement> chain, String when) {
        long undefined = chain.stream()
                .filter(g -> {
                    if (!g.isDefined()) {
                        return true;
                    }
                    return g instanceof GeoPoint && !((GeoPoint) g).isFinite();
                })
                .count();
        if (undefined != 0) {
            throw new IllegalStateException(undefined + " chain objects are undefined " + when
                    + "; the timing would not be meaningful");
        }
    }
}
