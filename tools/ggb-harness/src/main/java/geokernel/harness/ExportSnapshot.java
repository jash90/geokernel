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
import java.util.regex.Pattern;
import java.util.zip.ZipFile;

import javax.imageio.ImageIO;

import org.geogebra.common.awt.MyImage;

import org.geogebra.common.factories.AwtFactoryCommon;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.LocalizationCommon;
import org.geogebra.common.jre.headless.Utf8Control;
import org.geogebra.common.jre.io.MyXMLioJre;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.StringTemplate;
import org.geogebra.common.kernel.geos.GeoAngle;
import org.geogebra.common.kernel.geos.GeoBoolean;
import org.geogebra.common.kernel.geos.GeoButton;
import org.geogebra.common.kernel.geos.GeoConic;
import org.geogebra.common.kernel.geos.GeoConicPart;
import org.geogebra.common.kernel.geos.GeoElement;
import org.geogebra.common.kernel.geos.GeoImage;
import org.geogebra.common.kernel.geos.GeoInputBox;
import org.geogebra.common.kernel.geos.GeoLocus;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.kernel.geos.GeoVector;
import org.geogebra.common.kernel.geos.HasSegmentStyle;
import org.geogebra.common.kernel.geos.PointProperties;
import org.geogebra.common.kernel.geos.Traceable;
import org.geogebra.common.kernel.kernelND.GeoCurveCartesianND;
import org.geogebra.common.main.settings.config.AppConfigDefault;
import org.geogebra.common.util.debug.Log;

/** Loads exported archives in fresh headless apps and snapshots their labelled objects. */
public final class ExportSnapshot {
    private ExportSnapshot() {
    }

    /**
     * Without {@code --lenient} every warning and error GeoGebra reports while loading stops the
     * run: an export must load cleanly. With it the reports are recorded in the snapshot
     * ({@code reported}) and loading goes on as it does in the application, which is how a
     * hand-written file with a malformed tag is read.
     */
    private static boolean lenient;

    public static void main(String[] args) throws Exception {
        lenient = args.length == 2 && "--lenient".equals(args[0]);
        if (args.length != (lenient ? 2 : 1)) {
            throw new IllegalArgumentException(
                    "usage: ExportSnapshot [--lenient] <directory of .ggb files>");
        }
        Path dir = Path.of(args[args.length - 1]);
        try (var files = Files.list(dir)) {
            for (Path file : files.filter(p -> p.getFileName().toString().endsWith(".ggb")).sorted().toList()) {
                Path target = file.resolveSibling(file.getFileName() + ".snapshot");
                try {
                    Files.writeString(target, Json.write(snapshot(file)), StandardCharsets.UTF_8);
                } catch (Exception | AssertionError error) {
                    if (!lenient) {
                        throw new IllegalStateException("loading " + file.getFileName(), error);
                    }
                    // The reader gave up the file: record that instead of a state.
                    Files.writeString(target, Json.write(Map.of("failed", error.toString())),
                            StandardCharsets.UTF_8);
                }
            }
        }
    }

    private static Map<String, Object> snapshot(Path file) throws Exception {
        try (var archive = new ZipFile(file.toFile())) {
            return snapshot(file, archive);
        }
    }

    private static void report(List<Object> reported, String message) {
        if (!lenient) {
            throw new IllegalStateException(message);
        }
        reported.add(message);
    }

    private static Map<String, Object> snapshot(Path file, ZipFile archive) throws Exception {
        List<Object> reported = new ArrayList<>();
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
                report(reported, message);
            }

            @Override
            public void showError(String message) {
                report(reported, message);
            }

            @Override
            public void showError(String message, String detail) {
                report(reported, message + ": " + detail);
            }
        };
        Log logger = new Log() {
            @Override
            public void print(Level level, Object message) {
                String text = Objects.toString(message).toLowerCase(Locale.ROOT);
                if (level.compareTo(Level.WARN) <= 0 || message instanceof Throwable
                        || text.contains("unknown tag")) {
                    report(reported, level + ": " + message);
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
        Map<String, Object> snapshot = new LinkedHashMap<>();
        snapshot.put("objects", objects);
        snapshot.put("styles", styles);
        snapshot.put("settings", settings);
        snapshot.put("reported", reported);
        return snapshot;
    }

    /**
     * The presentation the reader gave an object, through GeoGebra's getters. A property that
     * does not apply to the class of the object is {@code null}.
     */
    private static Map<String, Object> describeStyle(GeoElement geo) {
        StringTemplate xml = StringTemplate.xmlTemplate;
        Map<String, Object> fields = new LinkedHashMap<>();
        fields.put("shown", geo.isSetEuclidianVisible());
        // The view bits GeoGebra itself would save: 1 not in the first view, 2 in the second,
        // 4 and 8 the 3D view, 16 and 32 the view of a plane.
        var views = Pattern.compile("<show [^>]*\\bev=\"(\\d+)\"").matcher(geo.getStyleXML());
        fields.put("views", views.find() ? Long.parseLong(views.group(1)) : 0L);
        fields.put("label_shown", geo.getLabelVisible());
        fields.put("layer", geo.getLayer());
        fields.put("label_mode", geo.getLabelMode());
        fields.put("label_offset", List.of(geo.labelOffsetX, geo.labelOffsetY));
        fields.put("caption", geo.getRawCaption());
        var color = geo.getObjectColor();
        fields.put("color", List.of(color.getRed(), color.getGreen(), color.getBlue()));
        fields.put("alpha", finite(geo.getAlphaValue()));
        Map<String, Object> line = new LinkedHashMap<>();
        line.put("type", geo.getLineType());
        line.put("thickness", geo.getLineThickness());
        line.put("opacity", geo.getLineOpacity());
        line.put("hidden_type", geo.getLineTypeHidden());
        fields.put("line", line);
        Map<String, Object> fill = new LinkedHashMap<>();
        fill.put("type", geo.getFillType().name());
        fill.put("angle", geo.getHatchingAngle());
        fill.put("distance", geo.getHatchingDistance());
        fill.put("symbol", Objects.toString(geo.getFillSymbol(), ""));
        fill.put("inverse", geo.isInverseFill());
        fields.put("fill", fill);
        var dynamic = geo.getColorFunction();
        if (dynamic != null) {
            List<Object> channels = new ArrayList<>();
            for (int i = 0; i < dynamic.size(); i++) {
                channels.add(dynamic.get(i).getLabel(xml));
            }
            fields.put("dynamic_color", channels);
            fields.put("color_space", geo.getColorSpace());
        } else {
            fields.put("dynamic_color", null);
            fields.put("color_space", null);
        }
        var condition = geo.getShowObjectCondition();
        fields.put("condition", condition == null ? null : condition.getLabel(xml));
        var background = geo.getBackgroundColor();
        fields.put("background", background == null ? null : List.of(background.getRed(),
                background.getGreen(), background.getBlue(), background.getAlpha()));
        fields.put("fixed", geo.isLocked());
        fields.put("auxiliary", geo.isAuxiliaryObject());
        fields.put("selection_allowed", geo.isSelectionAllowed(null));
        fields.put("tooltip_mode", geo.getTooltipMode());
        fields.put("decoration", geo.getDecorationType());
        fields.put("fixed_size", geo.isFixedSize());
        fields.put("show_trimmed", geo.getShowTrimmedIntersectionLines());
        fields.put("breakpoint", geo.isConsProtocolBreakpoint());
        fields.put("algebra_label", geo.isAlgebraLabelVisible());
        fields.put("ordering", finite(geo.getOrdering()));
        var dynamicCaption = geo.getDynamicCaption();
        fields.put("dynamic_caption", dynamicCaption == null ? null : dynamicCaption.getLabel(xml));
        fields.put("draw_arrows", geo instanceof GeoLocus locus ? locus.isDrawArrows() : null);
        if (geo instanceof HasSegmentStyle ends) {
            fields.put("start_style", ends.getStartStyle().name());
            fields.put("end_style", ends.getEndStyle().name());
        }
        if (geo instanceof GeoVector vector) {
            fields.put("head_style", vector.getHeadStyle().ordinal());
        }
        fields.put("trace", geo instanceof Traceable t ? t.getTrace() : null);
        fields.put("point_size", geo instanceof PointProperties p ? p.getPointSize() : null);
        fields.put("point_style", geo instanceof PointProperties p ? p.getPointStyle() : null);
        fields.put("checkbox_fixed", geo instanceof GeoBoolean b ? b.isLockedPosition() : null);
        Map<String, Object> animation = new LinkedHashMap<>();
        boolean auto = geo instanceof GeoNumeric n && n.isAutoStep();
        animation.put("step", auto ? null : geo.getAnimationStep());
        animation.put("speed", geo.getAnimationSpeed());
        animation.put("mode", geo.getAnimationType());
        animation.put("playing", geo.isAnimating());
        fields.put("animation", animation);
        if (geo instanceof GeoNumeric n && n.isIndependent()) {
            Map<String, Object> slider = new LinkedHashMap<>();
            slider.put("min", n.isIntervalMinActive() ? n.getIntervalMin() : null);
            slider.put("max", n.isIntervalMaxActive() ? n.getIntervalMax() : null);
            slider.put("position", List.of(finite(n.getSliderX()), finite(n.getSliderY())));
            slider.put("width", finite(n.getSliderWidth()));
            slider.put("knob_size", finite(n.getSliderBlobSize()));
            slider.put("horizontal", n.isSliderHorizontal());
            slider.put("fixed", n.isLockedPosition());
            slider.put("absolute", n.isAbsoluteScreenLocActive());
            slider.put("show_algebra", n.isAVSliderOrCheckboxVisible());
            fields.put("slider", slider);
        }
        if (geo instanceof GeoAngle angle) {
            fields.put("angle_style", angle.getAngleStyle().getXmlVal());
            fields.put("emphasize_right_angle", angle.isEmphasizeRightAngle());
            fields.put("arc_size", angle.getArcSize());
        }
        if (geo instanceof GeoPoint p && p.getPath() instanceof GeoCurveCartesianND) {
            fields.put("curve_parameter", p.getPathParameter().t);
        }
        if (geo instanceof GeoText text) {
            fields.put("latex", text.isLaTeX());
            Map<String, Object> font = new LinkedHashMap<>();
            font.put("size", text.getFontSizeMultiplier());
            font.put("serif", text.isSerifFont());
            font.put("style", text.getFontStyle());
            fields.put("font", font);
            fields.put("decimals", text.useSignificantFigures() ? -1 : text.getPrintDecimals());
            fields.put("figures", text.useSignificantFigures() ? text.getPrintFigures() : -1);
            var start = text.getStartPoint();
            fields.put("screen_location", text.isAbsoluteScreenLocActive());
            fields.put("start_point", start == null ? null : start.isLabelSet() ? start.getLabel(xml)
                    : List.of(start.getInhomX(), start.getInhomY()));
        }
        return fields;
    }

    /** A number for the snapshot; the JSON writer has no form for a value that is not finite. */
    private static Object finite(double value) {
        return Double.isFinite(value) ? (Object) value : Double.toString(value);
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
