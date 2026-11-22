package geokernel.harness;

import org.geogebra.common.cas.CASparser;
import org.geogebra.common.factories.CASFactory;
import org.geogebra.common.jre.cas.giac.CASgiacJre;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.kernel.CASGenericInterface;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.geos.GeoNumeric;

/** Optional JNI backend for the headless reference app. */
public final class GiacOracle {
    private GiacOracle() {
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
