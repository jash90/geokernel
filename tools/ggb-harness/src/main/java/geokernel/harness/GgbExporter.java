package geokernel.harness;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

import org.geogebra.common.jre.io.MyXMLioJre;
import org.geogebra.common.main.App;

/**
 * Writes the construction file GeoGebra itself produces for every case of the
 * case inputs (after the script and any CAS view actions).
 *
 * <p>Usage: {@code GgbExporter <cases.json> <out dir> <sample step> <zip step>}.
 * For a case file whose group is {@code g}, case number {@code n} (counting from
 * 0 in file order) is written to {@code <out dir>/g/<case id>.xml} when
 * {@code n % sample step == 0}: the text {@code app.getXML()} returns, the
 * content of {@code geogebra.xml} in a saved file. When {@code n % zip step == 0}
 * the case is also written as {@code <case id>.ggb}, the zip archive of
 * {@code MyXMLioJre.writeGeoGebraFile} without a thumbnail. Nothing is edited
 * after GeoGebra has written it.
 *
 * <p>The zip writer stamps every entry with the current time, so a new archive
 * is written only when its entries differ from the file that is already there.
 * Running it twice therefore leaves every file byte-identical.
 *
 * <p>Nothing is ever deleted by a partial run (a sample step above 1). A full
 * run (sample step 1) removes the files of the group directories that it did
 * not produce, for example the zip files of a different zip step, and prints
 * each file it removes.
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
        Map<Path, Set<String>> produced = new TreeMap<>();
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
                if (input.get("casview") instanceof List<?> actions) {
                    session.casView(actions);
                }
            } catch (RuntimeException e) {
                throw new IllegalStateException("case " + id + " failed: " + e.getMessage(), e);
            }
            Path dir = outDir.resolve(group);
            Files.createDirectories(dir);
            Set<String> names = produced.computeIfAbsent(dir, d -> new HashSet<>());
            App app = session.kernel().getApplication();
            byte[] xml = app.getXML().getBytes(StandardCharsets.UTF_8);
            Files.write(dir.resolve(id + ".xml"), xml);
            names.add(id + ".xml");
            bytes += xml.length;
            written++;
            perGroup.merge(group, 1, Integer::sum);
            if (n % zipStep == 0) {
                ByteArrayOutputStream zip = new ByteArrayOutputStream();
                ((MyXMLioJre) app.getXMLio()).writeGeoGebraFile(zip, false);
                Path target = dir.resolve(id + ".ggb");
                if (!Files.exists(target) || !sameEntries(Files.readAllBytes(target), zip.toByteArray())) {
                    Files.write(target, zip.toByteArray());
                }
                names.add(id + ".ggb");
                zips++;
            }
        }
        if (sampleStep == 1) {
            for (Map.Entry<Path, Set<String>> e : produced.entrySet()) {
                try (var files = Files.list(e.getKey())) {
                    for (Path file : (Iterable<Path>) files::iterator) {
                        if (!e.getValue().contains(file.getFileName().toString())) {
                            System.out.println("pruned " + file);
                            Files.delete(file);
                        }
                    }
                }
            }
        }
        printSummary(casesFile, written, bytes, zips, perGroup);
    }

    /** The entries (name and content) of two zip archives are equal. */
    static boolean sameEntries(byte[] a, byte[] b) throws IOException {
        return entries(a).equals(entries(b));
    }

    private static Map<String, List<Byte>> entries(byte[] zip) throws IOException {
        Map<String, List<Byte>> out = new TreeMap<>();
        try (ZipInputStream in = new ZipInputStream(new ByteArrayInputStream(zip))) {
            for (ZipEntry e = in.getNextEntry(); e != null; e = in.getNextEntry()) {
                List<Byte> bytes = new ArrayList<>();
                for (byte x : in.readAllBytes()) {
                    bytes.add(x);
                }
                out.put(e.getName(), bytes);
            }
        }
        return out;
    }

    private static void printSummary(Path casesFile, int written, long bytes, int zips,
            Map<String, Integer> perGroup) {
        System.out.printf("%s: %d xml files (%d bytes), %d zip files %s%n", casesFile.getFileName(),
                written, bytes, zips, new ArrayList<>(perGroup.entrySet()));
    }
}
