package geokernel.harness;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Set;

import org.geogebra.common.euclidian.Drawable;
import org.geogebra.common.euclidian.EuclidianConstants;
import org.geogebra.common.euclidian.EuclidianController;
import org.geogebra.common.euclidian.EuclidianView;
import org.geogebra.common.gui.dialog.TextInputDialog;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.jre.headless.DialogManagerNoGui;
import org.geogebra.common.kernel.MyPoint;
import org.geogebra.common.kernel.geos.properties.FillType;
import org.geogebra.common.kernel.geos.GeoInputBox;
import org.geogebra.common.kernel.geos.GeoList;
import org.geogebra.common.kernel.geos.GeoLocus;
import org.geogebra.common.kernel.geos.GeoNumeric;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.kernel.kernelND.GeoPointND;
import org.geogebra.test.TestEvent;

/** Canvas observations that are outside the construction snapshot format. */
public final class CanvasReview {
    private CanvasReview() {
    }

    public static void main(String[] args) throws Exception {
        Map<?, ?> inputs = (Map<?, ?>) Json.parse(Files.readString(Path.of(args[0])));
        for (Object item : (List<?>) inputs.get("cases")) {
            Map<?, ?> entry = (Map<?, ?>) item;
            String id = (String) entry.get("id");
            switch (id) {
                case "canvas-reviewmixed-001" -> mixed(id, construct(entry));
                case "canvas-reviewselection-001" -> selection(id, entry);
                case "canvas-reviewpen-001" -> pen(id);
                case "canvas-parts-001" -> picking(id, construct(entry));
                case "canvas-reviewlocus-001" -> locusHits(id, construct(entry));
                case "canvas-reviewstrokes-001" -> strokes(id);
                case "canvas-reviewchart-001" -> chartHits(id, construct(entry));
                case "canvas-reviewrectangle-001", "canvas-reviewstretch-001" -> rectangle(id, construct(entry));
                case "canvas-reviewbounds-001" -> listBounds(id, construct(entry));
                default -> { }
            }
        }
        Map<?, ?> controls = (Map<?, ?>) Json.parse(Files.readString(Path.of(args[1])));
        for (Object item : (List<?>) controls.get("cases")) {
            Map<?, ?> entry = (Map<?, ?>) item;
            String id = (String) entry.get("id");
            switch (id) {
                case "controls-reviewopen-001" -> open(id, construct(entry), Path.of(args[2]));
                case "controls-reviewblur-001" -> submit(id, construct(entry));
                case "controls-reviewtext-001" -> editFirstCharacter(id, construct(entry));
                case "controls-reviewsliderclick-001" -> submitAndClickSlider(id, construct(entry));
                case "controls-reviewtab-001" -> tabbing(id, construct(entry));
                default -> { }
            }
        }
    }

    private static void open(String id, Session session, Path file) throws Exception {
        AppCommon app = (AppCommon) view(session).getApplication();
        GeoInputBox oldBox = (GeoInputBox) session.kernel().lookupLabel("field");
        app.setXML(Files.readString(file), true);
        GeoInputBox newBox = (GeoInputBox) session.kernel().lookupLabel("field");
        System.out.printf("%s reused editor object=%s, loaded a=%s%n", id, oldBox == newBox,
                ((GeoNumeric) session.kernel().lookupLabel("a")).getDouble());
    }

    private static void submit(String id, Session session) {
        GeoInputBox box = (GeoInputBox) session.kernel().lookupLabel("field");
        box.updateLinkedGeo("3.5");
        System.out.printf("%s linked submission: a=%s, b=%s%n", id,
                ((GeoNumeric) session.kernel().lookupLabel("a")).getDouble(),
                ((GeoNumeric) session.kernel().lookupLabel("b")).getDouble());
    }

    private static void editFirstCharacter(String id, Session session) {
        GeoInputBox box = (GeoInputBox) session.kernel().lookupLabel("field");
        String original = box.getTextForEditor();
        box.updateLinkedGeo(original.substring(1));
        String result = ((GeoText) session.kernel().lookupLabel("note")).getTextStringSafe();
        System.out.printf("%s editor length=%d, edited length=%d, suffix=%s%n", id,
                original.length(), result.length(), result.endsWith("Z"));
    }

    private static void submitAndClickSlider(String id, Session session) {
        EuclidianView view = view(session);
        GeoNumeric slider = (GeoNumeric) session.kernel().lookupLabel("a");
        slider.setSliderLocation(80, 60, true);
        slider.setSliderWidth(100, true);
        slider.setSliderFixed(true);
        slider.setValue(2);
        slider.updateRepaint();
        GeoInputBox box = (GeoInputBox) session.kernel().lookupLabel("field");
        box.updateLinkedGeo("3.5");
        click(view.getEuclidianController(), new TestEvent(178, 60));
        System.out.printf("%s submitted and clicked: a=%s%n", id, slider.getDouble());
    }

    private static void tabbing(String id, Session session) {
        var selection = view(session).getApplication().getSelectionManager();
        var boxes = selection.getEVFilteredTabbingStream().filter(GeoInputBox.class::isInstance)
                .map(geo -> geo.getLabelSimple()).toList();
        System.out.printf("%s headless keyboard order=%s%n", id, boxes);
    }

    private static Session construct(Map<?, ?> entry) {
        Session session = new Session();
        for (Object line : (List<?>) entry.get("script")) {
            session.run((String) line);
        }
        return session;
    }

    private static EuclidianView view(Session session) {
        AppCommon app = (AppCommon) session.kernel().getApplication();
        app.setAppletFlag(false);
        EuclidianView view = app.getActiveEuclidianView();
        view.setCoordSystem(200, 200, 50, 50);
        view.getEuclidianController().setDraggingDelay(0);
        return view;
    }

    private static void mixed(String id, Session session) {
        GeoList tail = (GeoList) session.kernel().lookupLabel("L");
        GeoList head = (GeoList) session.kernel().lookupLabel("M");
        System.out.printf("%s drawable: tail=%s, head=%s%n", id,
                tail.isDrawable(), head.isDrawable());
    }

    private static void selection(String id, Map<?, ?> entry) throws ReflectiveOperationException {
        List<?> script = (List<?>) entry.get("script");
        Session session = new Session();
        EuclidianView view = view(session);
        AppCommon app = (AppCommon) view.getApplication();
        EuclidianController controller = view.getEuclidianController();
        for (Object line : script.subList(0, 5)) {
            session.run((String) line);
        }
        app.setMode(EuclidianConstants.MODE_CREATE_LIST);
        TestEvent start = new TestEvent(175, 75);
        TestEvent end = new TestEvent(375, 225);
        controller.wrapMouseMoved(start);
        controller.wrapMousePressed(start);
        controller.wrapMouseDragged(end, true);
        controller.wrapMouseDragged(end, true);
        controller.wrapMouseReleased(end);
        GeoList list = (GeoList) session.kernel().lookupLabel("l1");
        if (list != null) {
            System.out.printf("%s rectangle members: %s%n", id,
                    java.util.stream.IntStream.range(0, list.size())
                            .mapToObj(i -> list.get(i).getLabelSimple()).toList());
        }

        // The headless dialog does not create text. Capture the point passed
        // by the real controller at the dialog boundary.
        var dialogField = AppCommon.class.getDeclaredField("dialogManager");
        dialogField.setAccessible(true);
        dialogField.set(app, new DialogManagerNoGui(app, new String[0]) {
            @Override
            public TextInputDialog createTextDialog(GeoText text, GeoPointND point, boolean world) {
                System.out.printf("%s tool text anchor: %s%n", id,
                        point.getLabelSimple());
                return null;
            }
        });
        app.setMode(EuclidianConstants.MODE_TEXT);
        click(controller, new TestEvent(200, 200));

        session = new Session();
        view = view(session);
        controller = view.getEuclidianController();
        for (int i : new int[] {0, 1, 7, 8, 9}) {
            session.run((String) script.get(i));
        }
        var b = session.kernel().lookupLabel("B");
        b.setEuclidianVisible(false);
        b.updateRepaint();
        view.getApplication().setMode(EuclidianConstants.MODE_LOCUS);
        for (String label : List.of("P", "Q")) {
            GeoPoint point = (GeoPoint) session.kernel().lookupLabel(label);
            click(controller, new TestEvent(view.toScreenCoordX(point.getInhomX()),
                    view.toScreenCoordY(point.getInhomY())));
        }
        var locus = session.kernel().getConstruction().getGeoSetConstructionOrder().stream()
                .filter(GeoLocus.class::isInstance).findFirst().orElse(null);
        System.out.printf("%s driver-first locus: %s%n", id,
                locus != null && locus.isDefined());
    }

    private static void click(EuclidianController controller, TestEvent event) {
        controller.wrapMouseMoved(event);
        controller.wrapMousePressed(event);
        controller.wrapMouseReleased(event);
    }

    private static void picking(String id, Session session) {
        EuclidianView view = view(session);
        var sector = session.kernel().lookupLabel("s");
        sector.setAlphaValue(0);
        sector.updateRepaint();
        Drawable drawSector = (Drawable) view.getDrawableFor(sector);
        boolean unfilled = drawSector.hit(225, 175, 3);
        sector.setFillType(FillType.HATCH);
        sector.updateRepaint();
        boolean hatched = drawSector.hit(225, 175, 3);

        System.out.printf("%s sector interior unfilled=%s, hatched=%s%n", id, unfilled, hatched);
        locusHits(id, session);
    }

    private static void locusHits(String id, Session session) {
        EuclidianView view = view(session);
        GeoLocus locus = (GeoLocus) session.kernel().lookupLabel("l");
        Drawable drawLocus = (Drawable) view.getDrawableFor(locus);
        locus.getPoints().clear();
        locus.getPoints().add(new MyPoint(0, 0));
        locus.updateRepaint();
        boolean isolated = drawLocus.hit(200, 200, 3);

        locus.getPoints().clear();
        for (int i = 0; i < 10_000; i++) {
            locus.getPoints().add(new MyPoint(-2 + i / 10_000.0, 1));
        }
        MyPoint start = new MyPoint(0, 0);
        start.setLineTo(false);
        locus.getPoints().add(start);
        locus.getPoints().add(new MyPoint(2, 0));
        locus.updateRepaint();
        boolean late = drawLocus.hit(250, 200, 3);
        System.out.printf("%s locus isolated=%s, late=%s%n", id, isolated, late);
    }

    private static void strokes(String id) {
        Session session = new Session();
        EuclidianView view = view(session);
        AppCommon app = (AppCommon) view.getApplication();
        app.setMode(EuclidianConstants.MODE_PEN);
        EuclidianController controller = view.getEuclidianController();
        for (int y : new int[] {200, 300}) {
            TestEvent start = new TestEvent(200, y);
            TestEvent bend = new TestEvent(250, y - 50);
            TestEvent end = new TestEvent(300, y);
            controller.wrapMouseMoved(start);
            controller.wrapMousePressed(start);
            controller.wrapMouseDragged(bend, true);
            controller.wrapMouseDragged(end, true);
            controller.wrapMouseReleased(end);
        }
        var strokes = session.kernel().getConstruction().getGeoSetConstructionOrder().stream()
                .filter(GeoLocus.class::isInstance).map(GeoLocus.class::cast).toList();
        double low = strokes.stream().flatMap(stroke -> stroke.getPoints().stream())
                .mapToDouble(point -> point.y).filter(Double::isFinite).min().orElse(Double.NaN);
        double high = strokes.stream().flatMap(stroke -> stroke.getPoints().stream())
                .mapToDouble(point -> point.y).filter(Double::isFinite).max().orElse(Double.NaN);
        System.out.printf("%s objects=%d, y range=[%s, %s]%n", id, strokes.size(), low, high);
    }

    private static void chartHits(String id, Session session) {
        EuclidianView view = view(session);
        long hits = java.util.stream.IntStream.range(0, 17)
                .filter(i -> listHit(session, view, "dots" + i)).count();
        System.out.printf("%s first dot hits=%d, 257-member plot=%s%n", id,
                hits, listHit(session, view, "longdots"));
    }

    private static boolean listHit(Session session, EuclidianView view, String label) {
        Drawable drawable = (Drawable) view.getDrawableFor(session.kernel().lookupLabel(label));
        return drawable.hit(250, 150, 3);
    }

    private static void rectangle(String id, Session session) {
        var construction = session.kernel().getConstruction();
        var original = Set.copyOf(construction.getGeoSetConstructionOrder());
        EuclidianView view = view(session);
        EuclidianController controller = view.getEuclidianController();
        view.getApplication().setMode(EuclidianConstants.MODE_CREATE_LIST);
        TestEvent start = new TestEvent(175, 125);
        TestEvent end = new TestEvent(275, 225);
        controller.wrapMouseMoved(start);
        controller.wrapMousePressed(start);
        controller.wrapMouseDragged(end, true);
        controller.wrapMouseDragged(end, true);
        controller.wrapMouseReleased(end);
        GeoList list = construction.getGeoSetConstructionOrder().stream()
                .filter(geo -> !original.contains(geo))
                .filter(GeoList.class::isInstance).map(GeoList.class::cast)
                .findFirst().orElse(null);
        System.out.printf("%s rectangle members=%s%n", id, list == null ? List.of()
                : java.util.stream.IntStream.range(0, list.size())
                        .mapToObj(i -> list.get(i).getLabelSimple()).toList());
    }

    private static void listBounds(String id, Session session) {
        EuclidianView view = view(session);
        view.setCoordSystem(0, 200, 2, 50);
        System.out.printf("%s last point of third list hit=%s%n", id,
                ((Drawable) view.getDrawableFor(session.kernel().lookupLabel("L2")))
                        .hit(512, 100, 3));
    }

    private static void pen(String id) {
        Session session = new Session();
        EuclidianView view = view(session);
        view.getApplication().setMode(EuclidianConstants.MODE_PEN);
        EuclidianController controller = view.getEuclidianController();
        TestEvent start = new TestEvent(200, 200);
        TestEvent bend = new TestEvent(250, 150);
        TestEvent end = new TestEvent(300, 200);
        controller.wrapMouseMoved(start);
        controller.wrapMousePressed(start);
        controller.wrapMouseDragged(bend, true);
        controller.wrapMouseDragged(end, true);
        controller.wrapMouseReleased(end);
        var stroke = session.kernel().lookupLabel("stroke1");
        if (stroke instanceof GeoLocus locus) {
            double top = locus.getPoints().stream().mapToDouble(p -> p.y)
                    .filter(Double::isFinite).max().orElse(Double.NaN);
            System.out.printf("%s pointer stroke peak y=%s%n", id, top);
        } else {
            System.out.printf("%s pointer stroke label: %s%n", id, stroke);
        }
    }
}
