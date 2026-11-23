package geokernel.harness;

import org.geogebra.common.factories.UtilFactory;
import org.geogebra.common.jre.headless.FlushableTimer;
import org.geogebra.common.jre.util.UtilFactoryJre;
import org.geogebra.common.kernel.prover.AbstractProverReciosMethod;
import org.geogebra.common.main.ProverSettings;
import org.geogebra.common.util.GTimer;
import org.geogebra.common.util.GTimerListener;
import org.geogebra.common.util.HttpRequest;
import org.geogebra.common.util.Prover;

/**
 * Optional symbolic prover for the headless reference app.
 *
 * <p>The headless app's prover never decides a statement: its {@code compute} is empty, so every
 * {@code Prove} is {@code false} and every {@code ProveDetails} is {@code {}}. The desktop app runs
 * {@code Prover.decideStatement} instead. With {@code -DggbHarness.prover} this class installs a
 * prover that does the same, synchronously and without a timeout, with the algebraic (Botana)
 * engine selected: the numerical engine and the external Wu prover of the desktop app are not on
 * the harness class path. The rest of the factory matches the headless app's own.
 */
final class ProverOracle {
    private ProverOracle() {
    }

    static boolean enabled() {
        return System.getProperty("ggbHarness.prover") != null;
    }

    static void configure() {
        if (!enabled()) {
            return;
        }
        if (!GiacOracle.enabled()) {
            throw new IllegalStateException("the prover oracle requires the native Giac oracle");
        }
        ProverSettings settings = ProverSettings.get();
        settings.proverEngine = "Botana";
        settings.proverTimeout = 0;
        UtilFactory.setPrototype(new UtilFactoryJre() {
            @Override
            public HttpRequest newHttpRequest() {
                return null;
            }

            @Override
            public GTimer newTimer(GTimerListener listener, int delay) {
                return new FlushableTimer(listener);
            }

            @Override
            public Prover newProver() {
                return new Prover() {
                    @Override
                    public void compute() {
                        decideStatement();
                    }

                    @Override
                    protected ProofResult openGeoProver(ProverEngine engine) {
                        return ProofResult.UNKNOWN;
                    }

                    @Override
                    protected AbstractProverReciosMethod getNewReciosProver() {
                        return null;
                    }
                };
            }
        });
    }
}
