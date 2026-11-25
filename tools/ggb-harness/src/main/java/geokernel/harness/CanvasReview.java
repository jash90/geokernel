package geokernel.harness;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

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
                default -> { }
            }
        }
        Session session = new Session();
        session.run("a = Slider(0, 8, 0.5)");
        session.run("b = 2*a");
        session.run("field = InputBox(a)");
        GeoInputBox box = (GeoInputBox) session.kernel().lookupLabel("field");
        box.updateLinkedGeo("3.5");
        System.out.printf("controls-inputbox-001 linked submission: a=%s, b=%s%n",
                ((GeoNumeric) session.kernel().lookupLabel("a")).getDouble(),
                ((GeoNumeric) session.kernel().lookupLabel("b")).getDouble());
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
        System.out.printf("%s sector interior unfilled=%s, hatched=%s; locus isolated=%s, late=%s%n",
                id, unfilled, hatched, isolated, late);
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
