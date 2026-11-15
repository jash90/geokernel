package geokernel.harness;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.main.settings.config.AppConfigDefault;

/**
 * Loads saved construction files into a fresh GeoGebra, the way its reader
 * does, and prints the numbers it computes.
 *
 * <p>Usage: {@code Reload <file.xml> <label>...}. For every label that names a
 * number the line {@code <file name> <label> = <value>} is printed; nothing of
 * the file is edited.
 */
public final class Reload {

    private Reload() {
    }

    public static void main(String[] args) throws IOException {
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
            if (!(geo instanceof GeoNumeric)) {
                throw new IllegalArgumentException(args[i] + " is not a number in " + name);
            }
            System.out.println(name + " " + args[i] + " = " + ((GeoNumeric) geo).getDouble());
        }
    }
}
