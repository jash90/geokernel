package geokernel.harness;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.LinkedHashMap;
import java.util.Map;

import org.geogebra.common.cas.CASparser;
import org.geogebra.common.cas.CasEvaluation;
import org.geogebra.common.factories.CASFactory;
import org.geogebra.common.jre.cas.giac.CASgiacJre;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.kernel.CASGenericInterface;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.StringTemplate;
import org.geogebra.common.kernel.arithmetic.ValidExpression;
import org.geogebra.common.kernel.geos.GeoNumeric;

/** Optional JNI backend for the headless reference app. */
public final class GiacOracle {
    private GiacOracle() {
    }

    /** Case being run, recorded with every transcript line. */
    static volatile String currentCase = "";

    /**
     * With -DggbHarness.giacLog=<file>, every Giac exchange is appended to that
     * file as one JSON line: "translate" lines give the GeoGebra command text,
     * the Giac input built from it and the processed result; "raw" lines give
     * each string sent to Giac's caseval and what Giac printed.
     */
    private static void log(Map<String, Object> line) {
        String file = System.getProperty("ggbHarness.giacLog");
        if (file == null) {
            return;
        }
        line.put("case", currentCase);
        try {
            Files.writeString(Path.of(file), Json.writeLine(line) + "\n", StandardCharsets.UTF_8,
                    StandardOpenOption.CREATE, StandardOpenOption.APPEND);
        } catch (IOException e) {
            throw new IllegalStateException(e);
        }
    }

    private static final class FactoryHolder {
        private static final CASFactory FACTORY = load();

        private static CASFactory load() {
            System.load(System.getProperty("ggbHarness.giac"));
            boolean useThread = !System.getProperty("os.name").equals("Linux");
            return new CASFactory() {
                @Override
                public CASGenericInterface newGiac(CASparser parser, Kernel kernel) {
                    return new CASgiacJre(parser) {
                        @Override
                        protected boolean useThread() {
                            return useThread;
                        }

                        @Override
                        protected void debug(String prefix, String giacString) {
                            super.debug(prefix, giacString);
                            Map<String, Object> line = new LinkedHashMap<>();
                            line.put("event", "raw");
                            line.put("kind", prefix.trim());
                            line.put("text", giacString);
                            log(line);
                        }

                        @Override
                        protected CasEvaluation translateAndEvaluateCAS(
                                ValidExpression casInput, StringTemplate tpl) {
                            String ggb = casInput.toString(StringTemplate.defaultTemplate);
                            CasEvaluation result = super.translateAndEvaluateCAS(casInput, tpl);
                            Map<String, Object> line = new LinkedHashMap<>();
                            line.put("event", "translate");
                            line.put("ggb", ggb);
                            line.put("result", result.getValue());
                            log(line);
                            return result;
                        }
                    };
                }
            };
        }
    }

    static boolean enabled() {
        return System.getProperty("ggbHarness.giac") != null;
    }

    static void configure(AppCommon app) {
        if (enabled()) {
            app.setCASFactory(FactoryHolder.FACTORY);
        }
        ProverOracle.configure();
    }

    public static void main(String[] args) {
        // Going through the algebra processor exercises the same CAS path as golden cases.
        if (!(new Session().run("result = NextPrime(100)") instanceof GeoNumeric result)
                || result.getDouble() != 101) {
            throw new IllegalStateException("NextPrime(100) did not return 101");
        }
        System.out.println("Giac oracle returned 101 for NextPrime(100)");
    }
}
