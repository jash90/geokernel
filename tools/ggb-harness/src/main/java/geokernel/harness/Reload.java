package geokernel.harness;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.kernel.StringTemplate;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.main.settings.config.AppConfigDefault;

/**
 * Loads saved construction files into a fresh GeoGebra, the way its reader
 * does, and prints the values it computes.
 *
 * <p>Usage: {@code Reload <file.xml> <label>...}. For every requested label the line {@code <file name> <label> = <value>} is printed; nothing of
 * the file is edited.
 */
public final class Reload {

    private Reload() {
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2) {
            System.err.println("usage: Reload <file.xml> <label>...");
            System.exit(2);
        }
        LocalizationCommon loc = new LocalizationCommon(2);
        loc.setResourceBundleControl(new Utf8Control());
        AppCommon app = new AppCommon(loc, new AwtFactoryCommon(), new AppConfigDefault());
        String xml = Files.readString(Path.of(args[0]), StandardCharsets.UTF_8);
        app.getXMLio().processXMLString(xml, true, false);
        String name = Path.of(args[0]).getFileName().toString();
        for (int i = 1; i < args.length; i++) {
            GeoElement geo = app.getKernel().lookupLabel(args[i]);
            if (geo == null) {
                throw new IllegalArgumentException(args[i] + " is missing in " + name);
            }
            String value = geo instanceof GeoNumeric number ? Double.toString(number.getDouble())
                    : geo.toValueString(StringTemplate.maxPrecision);
            System.out.println(name + " " + args[i] + " = " + value);
        }
    }
}
