//! The texts the user sees, in one place (Polish today): the window's
//! labels, the tool names and the tool hints. Each is a `&'static str`
//! chosen by a `match` on an enum, so reading one costs nothing at run time
//! and nothing is formatted per frame. A second language becomes an
//! additive change: one more table over the same keys.
//!
//! Not here yet (texts with numbers or names in them, built when something
//! happens, not per frame): the notices of the workbench, the import and
//! the restore, the row menu's "delete together with", the load error, and
//! the translations of kernel errors in `errors.rs`.

use crate::tools::Tool;

/// A label, button name or prompt of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    Undo,
    Redo,
    Menu,
    File,
    SaveAsGgb,
    SavePath,
    SaveFile,
    SaveFailed,
    Axes,
    Grid,
    StandardView,
    ShowAllObjects,
    ZoomIn,
    ZoomOut,
    Fullscreen,
    PlayAnimation,
    PauseAnimation,
    StyleBar,
    Hide,
    Show,
    HideObject,
    ShowObject,
    Delete,
    CloseKeyboard,
    OpenKeyboard,
    LeaveTool,
    Close,
    CloseNotice,
    /// The notice of a refused drag of a dependent point.
    PointDependsOnOthers,
    Cancel,
    Ok,
    NotANumber,
    InputHint,
    KeyBackspace,
    KeyEnter,
    KeySpace,
    KeyLeft,
    KeyRight,
    KeyShift,
    Radius,
    VertexCount,
    AngleInDegrees,
    Scale,
    /// The value of an undefined object.
    Undefined,
    /// The value of an object of a kind the window cannot draw yet.
    NotDrawnYet,
}

/// The text of a window label.
pub const fn label(key: Label) -> &'static str {
    match key {
        Label::Undo => "Cofnij",
        Label::Redo => "Ponów",
        Label::Menu => "Menu",
        Label::File => "Plik",
        Label::SaveAsGgb => "Zapisz jako .ggb",
        Label::SavePath => "Ścieżka do nowego pliku",
        Label::SaveFile => "Zapisz",
        Label::SaveFailed => "Nie udało się zapisać pliku",
        Label::Axes => "Osie",
        Label::Grid => "Siatka",
        Label::StandardView => "Widok standardowy",
        Label::ShowAllObjects => "Pokaż wszystkie obiekty",
        Label::ZoomIn => "Powiększ",
        Label::ZoomOut => "Pomniejsz",
        Label::Fullscreen => "Pełny ekran",
        Label::PlayAnimation => "Odtwórz animację",
        Label::PauseAnimation => "Wstrzymaj animację",
        Label::StyleBar => "Pasek stylu",
        Label::Hide => "Ukryj",
        Label::Show => "Pokaż",
        Label::HideObject => "Ukryj obiekt",
        Label::ShowObject => "Pokaż obiekt",
        Label::Delete => "Usuń",
        Label::CloseKeyboard => "Zamknij klawiaturę",
        Label::OpenKeyboard => "Pokaż klawiaturę",
        Label::LeaveTool => "Zakończ",
        Label::Close => "Zamknij",
        Label::CloseNotice => "Zamknij komunikat",
        Label::PointDependsOnOthers => {
            "Tego punktu nie można przesunąć: zależy od innych obiektów."
        }
        Label::Cancel => "Anuluj",
        Label::Ok => "OK",
        Label::NotANumber => "To nie jest liczba",
        Label::InputHint => "Wprowadź…",
        Label::KeyBackspace => "Usuń znak",
        Label::KeyEnter => "Zatwierdź",
        Label::KeySpace => "Spacja",
        Label::KeyLeft => "W lewo",
        Label::KeyRight => "W prawo",
        Label::KeyShift => "Wielkie litery",
        Label::Radius => "Promień",
        Label::VertexCount => "Liczba wierzchołków",
        Label::AngleInDegrees => "Kąt w stopniach",
        Label::Scale => "Skala",
        Label::Undefined => "niezdefiniowany",
        Label::NotDrawnYet => "(ten rodzaj obiektu nie jest jeszcze rysowany)",
    }
}

/// The tool's name in the drop-down list and the tool hint.
pub const fn tool_caption(tool: Tool) -> &'static str {
    match tool {
        Tool::Move => "Przesuń",
        Tool::Point => "Punkt",
        Tool::Intersect => "Przecięcie",
        Tool::Midpoint => "Środek",
        Tool::Line => "Prosta",
        Tool::Segment => "Odcinek",
        Tool::Ray => "Półprosta",
        Tool::Vector => "Wektor",
        Tool::Perpendicular => "Prostopadła",
        Tool::Parallel => "Równoległa",
        Tool::Bisector => "Symetralna",
        Tool::AngleBisector => "Dwusieczna",
        Tool::Tangents => "Styczne",
        Tool::Polar => "Biegunowa",
        Tool::Circle => "Okrąg: środek i punkt",
        Tool::CircleRadius => "Okrąg: środek i promień",
        Tool::Circle3 => "Okrąg przez trzy punkty",
        Tool::Polygon => "Wielokąt",
        Tool::RegularPolygon => "Wielokąt foremny",
        Tool::Angle => "Kąt",
        Tool::Distance => "Odległość lub długość",
        Tool::ReflectLine => "Symetria osiowa",
        Tool::ReflectPoint => "Symetria środkowa",
        Tool::ReflectCircle => "Inwersja",
        Tool::Rotate => "Obrót",
        Tool::Translate => "Przesunięcie o wektor",
        Tool::Dilate => "Jednokładność",
        Tool::Slider => "Suwak",
        Tool::Checkbox => "Pole wyboru",
        Tool::MoveView => "Przesuń widok",
        Tool::ZoomIn => "Powiększ",
        Tool::ZoomOut => "Pomniejsz",
        Tool::ShowHide => "Pokaż lub ukryj",
        Tool::Delete => "Usuń",
        Tool::VectorFromPoint => "Wektor od punktu",
        Tool::Polyline => "Łamana",
        Tool::VectorPolygon => "Wielokąt wektorowy",
        Tool::Semicircle => "Półokrąg",
        Tool::CircularArc => "Łuk: środek i dwa punkty",
        Tool::CircumcircularArc => "Łuk przez trzy punkty",
        Tool::CircularSector => "Wycinek: środek i punkty",
        Tool::CircumcircularSector => "Wycinek przez trzy punkty",
        Tool::Compasses => "Cyrkiel",
        Tool::Ellipse => "Elipsa",
        Tool::Hyperbola => "Hiperbola",
        Tool::Parabola => "Parabola",
        Tool::Conic5 => "Stożkowa przez pięć punktów",
        Tool::Area => "Pole",
        Tool::Slope => "Nachylenie",
    }
}

/// What the next click of a tool should be (chosen in `tools::recipe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    DragPoint,
    ClickToPlacePoint,
    FirstObject,
    SecondObject,
    TwoPointsSegmentOrCircle,
    SecondPoint,
    FirstPointOfLine,
    SecondPointOfLine,
    FirstEndOfSegment,
    SecondEndOfSegment,
    StartOfRay,
    PointOnRay,
    StartOfVector,
    EndOfVector,
    PointOrLine,
    Line,
    Point,
    TwoPointsOrSegment,
    ThreePointsOrTwoLines,
    NextPoint,
    SecondLine,
    PointOrLineThenCircle,
    Circle,
    PointThenCircle,
    CenterOfCircle,
    PointOnCircle,
    ThreePointsOfCircle,
    FirstVertex,
    NextVertex,
    NextVertexOrClose,
    SecondVertex,
    AngleObjects,
    VertexOfAngle,
    PointOnSecondArm,
    DistanceObjects,
    ObjectToTransform,
    MirrorLine,
    MirrorPoint,
    CenterOfRotation,
    CenterOfDilation,
    Vector,
    InversionCircle,
    PlaceControl,
    DragView,
    ClickToZoomIn,
    ClickToZoomOut,
    ObjectToShowOrHide,
    ObjectToDelete,
    StartOfArc,
    EndOfArc,
    FirstFocus,
    SecondFocus,
    Focus,
    Directrix,
    FocusOrDirectrix,
    PointOnConic,
    FivePoints,
    NextVertexOrFinish,
    VectorPolygonStart,
    AreaObject,
    SlopeObject,
    PointOrVector,
}

/// The text of a tool hint.
pub const fn hint(key: Hint) -> &'static str {
    match key {
        Hint::DragPoint => "Przeciągnij punkt, aby go przesunąć",
        Hint::ClickToPlacePoint => "Kliknij, aby umieścić punkt",
        Hint::FirstObject => "Kliknij pierwszy obiekt",
        Hint::SecondObject => "Kliknij drugi obiekt",
        Hint::TwoPointsSegmentOrCircle => "Kliknij dwa punkty, odcinek albo okrąg",
        Hint::SecondPoint => "Kliknij drugi punkt",
        Hint::FirstPointOfLine => "Kliknij pierwszy punkt prostej",
        Hint::SecondPointOfLine => "Kliknij drugi punkt prostej",
        Hint::FirstEndOfSegment => "Kliknij pierwszy koniec odcinka",
        Hint::SecondEndOfSegment => "Kliknij drugi koniec odcinka",
        Hint::StartOfRay => "Kliknij początek półprostej",
        Hint::PointOnRay => "Kliknij punkt na półprostej",
        Hint::StartOfVector => "Kliknij początek wektora",
        Hint::EndOfVector => "Kliknij koniec wektora",
        Hint::PointOrLine => "Kliknij punkt lub prostą",
        Hint::Line => "Kliknij prostą",
        Hint::Point => "Kliknij punkt",
        Hint::TwoPointsOrSegment => "Kliknij dwa punkty albo odcinek",
        Hint::ThreePointsOrTwoLines => "Kliknij trzy punkty albo dwie proste",
        Hint::NextPoint => "Kliknij kolejny punkt",
        Hint::SecondLine => "Kliknij drugą prostą",
        Hint::PointOrLineThenCircle => "Kliknij punkt lub prostą, potem okrąg",
        Hint::Circle => "Kliknij okrąg",
        Hint::PointThenCircle => "Kliknij punkt, potem okrąg",
        Hint::CenterOfCircle => "Kliknij środek okręgu",
        Hint::PointOnCircle => "Kliknij punkt na okręgu",
        Hint::ThreePointsOfCircle => "Kliknij trzy punkty okręgu",
        Hint::FirstVertex => "Kliknij pierwszy wierzchołek",
        Hint::NextVertex => "Kliknij kolejny wierzchołek",
        Hint::NextVertexOrClose => "Kliknij kolejny wierzchołek albo pierwszy, aby zamknąć",
        Hint::SecondVertex => "Kliknij drugi wierzchołek",
        Hint::AngleObjects => "Wybierz trzy punkty, dwie proste, dwa wektory lub wielokąt",
        Hint::VertexOfAngle => "Kliknij wierzchołek kąta",
        Hint::PointOnSecondArm => "Kliknij punkt na drugim ramieniu",
        Hint::DistanceObjects => "Kliknij dwa punkty, punkt i prostą, odcinek albo okrąg",
        Hint::ObjectToTransform => "Kliknij obiekt do przekształcenia",
        Hint::MirrorLine => "Kliknij oś symetrii",
        Hint::MirrorPoint => "Kliknij środek symetrii",
        Hint::CenterOfRotation => "Kliknij środek obrotu",
        Hint::CenterOfDilation => "Kliknij środek jednokładności",
        Hint::Vector => "Kliknij wektor",
        Hint::InversionCircle => "Kliknij okrąg inwersji",
        Hint::PlaceControl => "Kliknij w widoku grafiki, aby ustawić położenie",
        Hint::DragView => "Przeciągnij, aby przesunąć widok",
        Hint::ClickToZoomIn => "Kliknij, aby powiększyć",
        Hint::ClickToZoomOut => "Kliknij, aby pomniejszyć",
        Hint::ObjectToShowOrHide => "Kliknij obiekt, aby go pokazać lub ukryć",
        Hint::ObjectToDelete => "Kliknij obiekt do usunięcia",
        Hint::StartOfArc => "Kliknij początek łuku",
        Hint::EndOfArc => "Kliknij koniec łuku",
        Hint::FirstFocus => "Kliknij pierwsze ognisko",
        Hint::SecondFocus => "Kliknij drugie ognisko",
        Hint::Focus => "Kliknij ognisko",
        Hint::Directrix => "Kliknij kierownicę",
        Hint::FocusOrDirectrix => "Kliknij ognisko lub kierownicę",
        Hint::PointOnConic => "Kliknij punkt na krzywej",
        Hint::FivePoints => "Kliknij pięć punktów krzywej",
        Hint::NextVertexOrFinish => "Kliknij kolejny punkt albo pierwszy, aby zakończyć",
        Hint::VectorPolygonStart => "Kliknij pierwszy wierzchołek lub wielokąt",
        Hint::AreaObject => "Kliknij wielokąt, stożkową lub wycinek",
        Hint::SlopeObject => "Kliknij prostą, odcinek, półprostą lub wykres funkcji",
        Hint::PointOrVector => "Kliknij punkt lub wektor",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::GROUPS;

    #[test]
    fn every_tool_has_a_distinct_caption() {
        let tools: Vec<Tool> = GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
        let mut captions: Vec<&str> = tools.iter().map(|&t| tool_caption(t)).collect();
        assert!(captions.iter().all(|c| !c.is_empty()));
        captions.sort_unstable();
        captions.dedup();
        assert_eq!(captions.len(), tools.len());
    }

    #[test]
    fn texts_are_constants() {
        // Usable in constants: nothing is looked up or formatted.
        const UNDO: &str = label(Label::Undo);
        const DRAG: &str = hint(Hint::DragPoint);
        assert_eq!(UNDO, "Cofnij");
        assert_eq!(DRAG, "Przeciągnij punkt, aby go przesunąć");
    }
}
