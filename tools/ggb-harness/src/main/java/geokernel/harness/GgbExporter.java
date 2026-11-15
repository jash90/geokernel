package geokernel.harness;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import org.geogebra.common.jre.io.MyXMLioJre;

/**
 * Writes the construction file GeoGebra itself produces for every case of the
 * case inputs (the state right after the script has run).
 *
 * <p>Usage: {@code GgbExporter <cases.json> <out dir> <sample step> <zip step>}.
 * For a case file whose group is {@code g}, case number {@code n} (counting from
 * 0 in file order) is written to {@code <out dir>/g/<case id>.xml} when
 * {@code n % sample step == 0}: the text {@code app.getXML()} returns, the
 * content of {@code geogebra.xml} in a saved file. When {@code n % zip step == 0}
 * the case is also written as {@code <case id>.ggb}, the zip archive of
 * {@code MyXMLioJre.writeGeoGebraFile} without a thumbnail. Nothing is edited
 * after GeoGebra has written it.
 */
public final class GgbExporter {

    private GgbExporter() {
    }

    public static void main(String[] args) throws IOException {
        if (args.length != 4) {
            System.err.println(
                    "usage: GgbExporter <cases.json> <out dir> <sample step> <zip step>");
            System.exit(2);
        }
        Path casesFile = Path.of(args[0]);
        Path outDir = Path.of(args[1]);
        int sampleStep = Integer.parseInt(args[2]);
        int zipStep = Integer.parseInt(args[3]);
        Object parsed = Json.parse(Files.readString(casesFile, StandardCharsets.UTF_8));
        List<?> cases = (List<?>) ((Map<?, ?>) parsed).get("cases");
        int written = 0;
        int zips = 0;
        long bytes = 0;
        Map<String, Integer> perGroup = new TreeMap<>();
        for (int n = 0; n < cases.size(); n++) {
            if (n % sampleStep != 0) {
                continue;
            }
            Map<?, ?> input = (Map<?, ?>) cases.get(n);
            String id = (String) input.get("id");
            String group = (String) input.get("group");
            Session session = new Session();
            try {
                for (Object line : (List<?>) input.get("script")) {
                    session.run((String) line);
                }
            } catch (RuntimeException e) {
                throw new IllegalStateException("case " + id + " failed: " + e.getMessage(), e);
            }
            Path dir = outDir.resolve(group);
            Files.createDirectories(dir);
            byte[] xml = session.app().getXML().getBytes(StandardCharsets.UTF_8);
            Files.write(dir.resolve(id + ".xml"), xml);
            bytes += xml.length;
            written++;
            perGroup.merge(group, 1, Integer::sum);
            if (n % zipStep == 0) {
                ByteArrayOutputStream zip = new ByteArrayOutputStream();
                ((MyXMLioJre) session.app().getXMLio()).writeGeoGebraFile(zip, false);
                Files.write(dir.resolve(id + ".ggb"), zip.toByteArray());
                zips++;
            }
        }
        System.out.printf("%s: %d xml files (%d bytes), %d zip files %s%n", casesFile.getFileName(),
                written, bytes, zips, new ArrayList<>(perGroup.entrySet()));
    }
}
