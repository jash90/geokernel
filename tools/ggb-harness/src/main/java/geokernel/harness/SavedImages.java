package geokernel.harness;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import java.util.zip.ZipOutputStream;
import javax.imageio.IIOImage;
import javax.imageio.ImageIO;

import org.geogebra.common.awt.AwtFactory;
import org.geogebra.common.kernel.Kernel;
import org.geogebra.common.kernel.StringTemplate;
import org.geogebra.common.kernel.geos.GeoButton;
import org.geogebra.common.kernel.geos.GeoImage;
import org.geogebra.common.kernel.geos.GeoPoint;

/** Synthetic assets for the headless saver, whose bitmap encoder is a stub. */
final class SavedImages {

    private static final int WIDTH = 12;
    private static final int HEIGHT = 8;

    private SavedImages() {
    }

    static GeoImage create(Kernel kernel, Map<?, ?> spec, Map<String, byte[]> assets)
            throws IOException {
        var image = new GeoImage(kernel.getConstruction());
        String name = (String) spec.get("file");
        if (!assets.containsKey(name)) {
            String format = spec.get("format") instanceof String override
                    ? override : name.substring(name.lastIndexOf('.') + 1);
            assets.put(name, raster(format));
        }
        image.setImageFileName(name, WIDTH, HEIGHT);
        image.setLabel((String) spec.get("label"));
        image.setEuclidianVisible(true);
        image.setInBackground(Boolean.TRUE.equals(spec.get("background")));
        if (spec.get("alpha") instanceof Number alpha) {
            image.setAlphaValue(alpha.floatValue());
        }
        if (spec.get("screen") instanceof List<?> screen) {
            image.setAbsoluteScreenLocActive(true);
            image.setAbsoluteScreenLoc(((Number) screen.get(0)).intValue(),
                    ((Number) screen.get(1)).intValue());
        }
        if (spec.get("corners") instanceof List<?> corners) {
            for (int i = 0; i < corners.size(); i++) {
                if (corners.get(i) instanceof String label) {
                    image.setCorner((GeoPoint) kernel.lookupLabel(label), i);
                } else if (corners.get(i) instanceof List<?> coords) {
                    image.setRealWorldCoord(number(coords.get(0)), number(coords.get(1)), i);
                }
            }
        }
        if (Boolean.TRUE.equals(spec.get("centered"))) {
            image.setCentered(true);
        }
        if (spec.get("crop") instanceof List<?> crop) {
            var rect = AwtFactory.getPrototype().newRectangle2D();
            rect.setRect(number(crop.get(0)), number(crop.get(1)), number(crop.get(2)),
                    number(crop.get(3)));
            image.setCropBoxRelative(rect);
            image.setCropped(true);
        }
        image.updateCascade();
        return image;
    }

    static void setIcon(GeoButton button, String name, Map<String, byte[]> assets)
            throws IOException {
        assets.put(name, raster("png"));
        button.getGraphicsAdapter().setImageFileNameOnly(name);
        button.getGraphicsAdapter().setImageOnly(
                button.getKernel().getApplication().getExternalImageAdapter(name, WIDTH, HEIGHT));
    }

    private static double number(Object value) {
        return ((Number) value).doubleValue();
    }

    static Map<String, Object> describe(GeoImage image) {
        Map<String, Object> obj = new LinkedHashMap<>();
        obj.put("type", "image");
        obj.put("defined", image.isDefined());
        obj.put("text", image.toValueString(StringTemplate.defaultTemplate));
        obj.put("file", image.getImageFileName());
        obj.put("size", List.of(image.getFillImage().getWidth(), image.getFillImage().getHeight()));
        obj.put("background", image.isInBackground());
        obj.put("alpha", image.getAlphaValue());
        if (image.isAbsoluteScreenLocActive()) {
            obj.put("screen", List.of(image.getAbsoluteScreenLocX(), image.getAbsoluteScreenLocY()));
        } else {
            var point = new GeoPoint(image.getConstruction());
            List<List<Double>> corners = new ArrayList<>();
            for (int i : new int[] {4, 3, 2, 1}) {
                image.calculateCornerPoint(point, i);
                corners.add(List.of(point.getInhomX(), point.getInhomY()));
            }
            obj.put("corners", corners);
        }
        if (image.isCropped()) {
            var rect = image.getCropBoxRelative();
            obj.put("crop", List.of(rect.getX(), rect.getY(), rect.getWidth(), rect.getHeight()));
        }
        return obj;
    }

    private static byte[] raster(String format) throws IOException {
        var first = new BufferedImage(WIDTH, HEIGHT, "png".equals(format)
                ? BufferedImage.TYPE_INT_ARGB : BufferedImage.TYPE_INT_RGB);
        for (int y = 0; y < HEIGHT; y++) {
            for (int x = 0; x < WIDTH; x++) {
                first.setRGB(x, y, x < WIDTH / 2 ? 0xffff0000 : 0x8000ff00);
            }
        }
        var bytes = new ByteArrayOutputStream();
        if ("gif".equals(format)) {
            var writer = ImageIO.getImageWritersByFormatName(format).next();
            try (var stream = ImageIO.createImageOutputStream(bytes)) {
                writer.setOutput(stream);
                writer.prepareWriteSequence(null);
                writer.writeToSequence(new IIOImage(first, null, null), null);
                var second = new BufferedImage(WIDTH, HEIGHT, BufferedImage.TYPE_INT_RGB);
                writer.writeToSequence(new IIOImage(second, null, null), null);
                writer.endWriteSequence();
            } finally {
                writer.dispose();
            }
        } else if (!ImageIO.write(first, format, bytes)) {
            throw new IOException("no image encoder for " + format);
        }
        return bytes.toByteArray();
    }

    static byte[] embed(byte[] archive, Map<String, byte[]> assets) throws IOException {
        if (assets.isEmpty()) {
            return archive;
        }
        var bytes = new ByteArrayOutputStream();
        try (var source = new ZipInputStream(new ByteArrayInputStream(archive));
                var target = new ZipOutputStream(bytes)) {
            for (ZipEntry entry; (entry = source.getNextEntry()) != null;) {
                target.putNextEntry(new ZipEntry(entry.getName()));
                source.transferTo(target);
                target.closeEntry();
            }
            for (var asset : assets.entrySet()) {
                target.putNextEntry(new ZipEntry(asset.getKey()));
                target.write(asset.getValue());
                target.closeEntry();
            }
        }
        return bytes.toByteArray();
    }
}
