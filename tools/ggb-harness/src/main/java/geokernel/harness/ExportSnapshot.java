package geokernel.harness;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.zip.ZipFile;

import javax.imageio.ImageIO;

import org.geogebra.common.awt.MyImage;

import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.jre.io.MyXMLioJre;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.geos.GeoButton;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoConicPart;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoImage;
import org.geogebra.common.kernel.geos.GeoInputBox;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.kernel.kernelND.GeoCurveCartesianND;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.common.util.debug.Log;

/** Loads exported archives in fresh headless apps and snapshots their labelled objects. */
public final class ExportSnapshot {
    private ExportSnapshot() {
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("usage: ExportSnapshot <directory of .ggb files>");
        }
        Path dir = Path.of(args[0]);
        try (var files = Files.list(dir)) {
            for (Path file : files.filter(p -> p.getFileName().toString().endsWith(".ggb")).sorted().toList()) {
                try {
                    Files.writeString(file.resolveSibling(file.getFileName() + ".snapshot"),
                            Json.write(snapshot(file)), StandardCharsets.UTF_8);
                } catch (Exception | AssertionError error) {
                    throw new IllegalStateException("loading " + file.getFileName(), error);
                }
            }
        }
    }

    private static Map<String, Object> snapshot(Path file) throws Exception {
        try (var archive = new ZipFile(file.toFile())) {
            return snapshot(file, archive);
        }
    }

    private static Map<String, Object> snapshot(Path file, ZipFile archive) throws Exception {
        LocalizationCommon loc = new LocalizationCommon(2);
        loc.setResourceBundleControl(new Utf8Control());
        AppCommon app = new AppCommon(loc, new AwtFactoryCommon(), new AppConfigDefault()) {
            // AppCommon's default image adapter uses placeholder dimensions.
            // Resolve the requested raster from the same archive as the XML reader.
            @Override
            public MyImage getExternalImageAdapter(String name, int width, int height) {
                if (name == null || name.isEmpty()) {
                    return super.getExternalImageAdapter(name, width, height);
                }
                var entry = Objects.requireNonNull(archive.getEntry(name), "missing raster " + name);
                try (var input = archive.getInputStream(entry)) {
                    var bitmap = Objects.requireNonNull(ImageIO.read(input), "unreadable raster " + name);
                    return super.getExternalImageAdapter(name, bitmap.getWidth(), bitmap.getHeight());
                } catch (IOException error) {
                    throw new UncheckedIOException(error);
                }
            }

            @Override
            protected void showErrorDialog(String message) {
                throw new IllegalStateException(message);
            }

            @Override
            public void showError(String message) {
                throw new IllegalStateException(message);
            }

            @Override
            public void showError(String message, String detail) {
                throw new IllegalStateException(message + ": " + detail);
            }
        };
        Log logger = new Log() {
            @Override
            public void print(Level level, Object message) {
                String text = Objects.toString(message).toLowerCase(Locale.ROOT);
                if (level.compareTo(Level.WARN) <= 0 || message instanceof Throwable
                        || text.contains("unknown tag")) {
                    throw new IllegalStateException(level + ": " + message);
                }
            }
        };
        logger.setLogLevel(Log.Level.TRACE);
        Log.setLogger(logger);
        try (InputStream in = Files.newInputStream(file)) {
            ((MyXMLioJre) app.getXMLio()).readZipFromInputStream(in, false);
        }

        Map<String, Object> objects = new LinkedHashMap<>();
        Map<String, Object> styles = new LinkedHashMap<>();
        for (GeoElement geo : app.getKernel().getConstruction().getGeoSetConstructionOrder()) {
            if (geo.isLabelSet()) {
                objects.put(geo.getLabelSimple(), describe(geo));
                styles.put(geo.getLabelSimple(), describeStyle(geo));
            }
        }
        var view = app.getSettings().getEuclidian(1);
        var kernel = app.getKernel();
        String angle = switch (kernel.getAngleUnit()) {
            case Kernel.ANGLE_RADIANT -> "radiant";
            case Kernel.ANGLE_DEGREES_MINUTES_SECONDS -> "degreesMinutesSeconds";
            default -> "degree";
        };
        Map<String, Object> settings = new LinkedHashMap<>();
        settings.put("origin", List.of(view.getXZero(), view.getYZero()));
        settings.put("scale", List.of(view.getXscale(), view.getYscale()));
        settings.put("axes", List.of(view.getShowAxes()[0], view.getShowAxes()[1]));
        settings.put("grid", view.getShowGrid());
        settings.put("angle", angle);
        settings.put("rounding", kernel.useSignificantFigures ? "figures" : "decimals");
        settings.put("digits", kernel.useSignificantFigures ? kernel.getPrintFigures() : kernel.getPrintDecimals());
        return Map.of("objects", objects, "styles", styles, "settings", settings);
    }

    private static Map<String, Object> describeStyle(GeoElement geo) {
        Map<String, Object> fields = new LinkedHashMap<>();
        fields.put("shown", geo.isSetEuclidianVisible());
        fields.put("label_shown", geo.getLabelVisible());
        fields.put("layer", geo.getLayer());
        fields.put("label_mode", geo.getLabelMode());
        fields.put("label_offset", List.of(geo.labelOffsetX, geo.labelOffsetY));
        fields.put("caption", geo.getRawCaption());
        var color = geo.getObjectColor();
        fields.put("color", List.of(color.getRed(), color.getGreen(), color.getBlue()));
        fields.put("line_style", List.of(geo.getLineType(), geo.getLineThickness()));
        if (geo instanceof GeoNumeric n && n.isIndependent()
                && (n.isIntervalMinActive() || n.isIntervalMaxActive())) {
            Map<String, Object> slider = new LinkedHashMap<>();
            slider.put("min", n.isIntervalMinActive() ? n.getIntervalMin() : null);
            slider.put("max", n.isIntervalMaxActive() ? n.getIntervalMax() : null);
            slider.put("step", n.isAutoStep() ? null : n.getAnimationStep());
            slider.put("speed", n.getAnimationSpeed());
            slider.put("mode", n.getAnimationType());
            slider.put("playing", n.isAnimating());
            slider.put("position", List.of(n.getSliderX(), n.getSliderY()));
            fields.put("slider", slider);
        }
        if (geo instanceof GeoPoint p && p.getPath() instanceof GeoCurveCartesianND) {
            fields.put("curve_parameter", p.getPathParameter().t);
        }
        if (geo instanceof GeoText text) {
            fields.put("latex", text.isLaTeX());
        }
        return fields;
    }

    private static Map<String, Object> describe(GeoElement geo) {
        if (geo instanceof GeoImage image) {
            return describeImage(image);
        }
        if (geo instanceof GeoInputBox box) {
            var link = box.getLinkedGeo();
            return Map.of("type", "textfield", "caption", Objects.toString(geo.getCaptionSimple(), ""),
                    "defined", geo.isDefined(), "link", link != null && link.isLabelSet() ? link.getLabelSimple() : "");
        }
        if (geo instanceof GeoButton) {
            return Map.of("type", "button", "caption", Objects.toString(geo.getCaptionSimple(), ""),
                    "defined", geo.isDefined());
        }
        String type = geo instanceof GeoConic conic && !(conic instanceof GeoConicPart)
                && conic.getType() != GeoConic.CONIC_CIRCLE ? "conic" : Session.typeOf(geo);
        return Session.describe(geo, type, true);
    }

    private static Map<String, Object> describeImage(GeoImage image) {
        var asset = Objects.requireNonNull(image.getFillImage(), "missing image " + image.getLabelSimple());
        Map<String, Object> fields = new LinkedHashMap<>();
        fields.put("type", "image");
        fields.put("defined", image.isDefined());
        fields.put("size", List.of(asset.getWidth(), asset.getHeight()));
        fields.put("background", image.isInBackground());
        fields.put("alpha", image.getAlphaValue());
        fields.put("interpolate", image.isInterpolate());
        fields.put("centered", image.isCentered());
        if (image.isCropped()) {
            var crop = image.getCropBoxRelative();
            fields.put("crop", List.of(crop.getX(), crop.getY(), crop.getWidth(), crop.getHeight()));
        }
        // The headless app has no drawable for an image. Read its stored
        // anchors, including their references, without asking for a quad.
        if (image.isAbsoluteScreenLocActive() && (image.getStartPoint() == null
                || image.getStartPoint().isAbsoluteStartPoint())) {
            fields.put("screen", List.of(image.getAbsoluteScreenLocX(), image.getAbsoluteScreenLocY()));
        } else {
            fields.put("screen_coords", image.isAbsoluteScreenLocActive());
            List<Object> corners = new ArrayList<>();
            for (int index : image.isCentered() ? new int[] {3, 1, 2} : new int[] {0, 1, 2}) {
                var point = image.getStartPoint(index);
                corners.add(point == null ? null : point.isLabelSet() ? point.getLabelSimple()
                        : List.of(point.getInhomX(), point.getInhomY()));
            }
            fields.put("anchors", corners);
        }
        return fields;
    }

}
