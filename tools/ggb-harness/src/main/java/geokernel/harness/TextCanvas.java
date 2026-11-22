package geokernel.harness;

import org.geogebra.common.awt.GRectangle;
import org.geogebra.common.euclidian.EuclidianConstants;
import org.geogebra.common.euclidian.EuclidianController;
import org.geogebra.common.euclidian.EuclidianView;
import org.geogebra.common.euclidian.draw.DrawText;
import org.geogebra.common.jre.headless.AppCommon;
import org.geogebra.common.kernel.geos.GeoPoint;
import org.geogebra.common.kernel.geos.GeoText;
import org.geogebra.common.plugin.EuclidianStyleConstants;
import org.geogebra.test.TestEvent;

/** Pointer and placement cases for the Classic graphics view. */
public final class TextCanvas {
    private TextCanvas() {
    }

    public static void main(String[] args) throws Exception {
        checkDrag("screen", false, true, false);
        checkDrag("world", false, false, false);
        checkDrag("point", true, false, false);
        checkDrag("screen point", true, true, false);
        checkDrag("command point", true, false, true);
        checkPixels();
    }

    private static EuclidianView view(Session session) {
        AppCommon app = (AppCommon) session.kernel().getApplication();
        app.setAppletFlag(false);
        EuclidianView view = app.getActiveEuclidianView();
        view.setCoordSystem(200.75, 180.75, 50, 50);
        view.getSettings().setPointCapturing(EuclidianStyleConstants.POINT_CAPTURING_OFF);
        view.getEuclidianController().setDraggingDelay(0);
        app.setMode(EuclidianConstants.MODE_MOVE);
        return view;
    }

    private static void checkDrag(String name, boolean anchored, boolean screen, boolean command)
            throws Exception {
        Session session = new Session();
        EuclidianView view = view(session);
        session.run("A = (1, 1)");
        session.run(command ? "t = Text(\"Canvas text\", A)" : "t = \"Canvas text\"");
        GeoPoint anchor = (GeoPoint) session.kernel().lookupLabel("A");
        GeoText text = (GeoText) session.kernel().lookupLabel("t");
        if (screen) {
            text.setAbsoluteScreenLocActive(true);
            if (!anchored) {
                text.setAbsoluteScreenLoc(150, 180);
            }
        }
        if (anchored && !command) {
            if (screen) {
                anchor.setCoords(150, 180, 1);
            }
            text.setStartPoint(anchor);
        } else if (!anchored && !screen) {
            text.setStartPoint(new GeoPoint(session.kernel().getConstruction(), 1, 1, 1));
        }
        text.setEuclidianVisible(true);
        text.updateRepaint();
        DrawText draw = (DrawText) view.getDrawableFor(text);
        GRectangle bounds = draw.getBounds();
        int x = (int) bounds.getX() + 20;
        int y = (int) (bounds.getY() + bounds.getHeight() / 2);
        check(draw.hit(x, y, 0), name + " misses its rectangle " + bounds);
        check(!draw.hit(x, (int) bounds.getY() - 1, 20), name + " accepts an outside point");
        check(!draw.hitLabel(x, y), name + " is classified as a label");
        EuclidianController controller = view.getEuclidianController();
        TestEvent from = new TestEvent(x, y);
        controller.wrapMouseMoved(from);
        controller.wrapMousePressed(from);
        TestEvent to = new TestEvent(x + 60, y - 35);
        controller.wrapMouseDragged(to, true);
        controller.wrapMouseDragged(to, true);
        controller.wrapMouseReleased(to);
        check(text.isSelected(), name + " was not selected");
        if (command) {
            check(text.labelOffsetX == 0 && text.labelOffsetY == 0,
                    name + " moved independently of A");
            check(text.getStartPoint().getInhomX() == 1 && text.getStartPoint().getInhomY() == 1,
                    name + " moved its start point");
            anchor.setCoords(2, 3, 1);
            anchor.updateCascade();
            check(text.getStartPoint().getInhomX() == 2 && text.getStartPoint().getInhomY() == 3,
                    name + " did not follow A");
        } else if (anchored) {
            check(text.getStartPoint() == anchor, name + " detached from A");
            check(anchor.getInhomX() == (screen ? 150 : 1), name + " moved A");
            check(anchor.getInhomY() == (screen ? 180 : 1), name + " moved A");
            check(text.labelOffsetX == (screen ? 0 : 60)
                    && text.labelOffsetY == (screen ? 0 : -35), name + " used an unexpected offset");
        } else if (screen) {
            check(text.getAbsoluteScreenLocX() == 210 && text.getAbsoluteScreenLocY() == 145,
                    name + " did not move by the screen delta");
        } else {
            check(Math.abs(text.getStartPoint().getInhomX() - 2.2) < 1e-9
                    && Math.abs(text.getStartPoint().getInhomY() - 1.7) < 1e-9,
                    name + " did not move by the world delta");
        }
        System.out.println(name + " text hit, selection and drag passed");
    }

    private static void checkPixels() {
        Session session = new Session();
        EuclidianView view = view(session);
        session.run("t = \"Canvas text\"");
        session.run("A = (1.016, 0.984)");
        session.run("u = Text(t, A)");
        GeoText origin = (GeoText) session.kernel().lookupLabel("t");
        GeoText point = (GeoText) session.kernel().lookupLabel("u");
        DrawText fromOrigin = (DrawText) view.getDrawableFor(origin);
        DrawText fromPoint = (DrawText) view.getDrawableFor(point);
        check(fromOrigin.getLabelX() == 200 && fromOrigin.getLabelY() == 180,
                "the implicit origin did not truncate fractional pixels");
        check(fromPoint.getLabelX() == 252 && fromPoint.getLabelY() == 132,
                "the projected anchor did not round fractional pixels: "
                        + fromPoint.getLabelX() + ", " + fromPoint.getLabelY());
        System.out.println("text integer placement passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
