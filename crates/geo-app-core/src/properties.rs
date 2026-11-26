// Property eligibility, order, and ranges follow GeoGebra
// (https://github.com/geogebra/geogebra), commit
// 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/desktop/desktop/src/main/java/org/geogebra/desktop/gui/dialog/PropertiesPanelD.java
//   source/desktop/desktop/src/main/java/org/geogebra/desktop/gui/properties/SliderPropertiesPanelD.java
//   source/shared/common/src/main/java/org/geogebra/common/gui/dialog/options/model/LineStyleModel.java
//   source/shared/common/src/main/java/org/geogebra/common/gui/dialog/options/model/LayerModel.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Inspector tabs and the selection captured when the inspector opens.

use crate::workbench::Workbench;
use geo_kernel::{Construction, Def, Id, Kind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Basic,
    Color,
    Style,
    Position,
    Advanced,
}

impl Tab {
    pub const ALL: [Self; 5] = [
        Self::Basic,
        Self::Color,
        Self::Style,
        Self::Position,
        Self::Advanced,
    ];
}

pub const MAX_EDIT_TARGETS: usize = 512;

pub struct Dialog {
    pub tab: Tab,
    pub draft: Option<(Field, String)>,
    pub fields: Vec<Field>,
    pub error: Option<EditError>,
    targets: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditError {
    SelectionTooLarge,
    MissingTarget,
    InvalidValue,
}

impl Dialog {
    pub fn open(wb: &Workbench, ids: impl IntoIterator<Item = Id>) -> Option<Self> {
        let c = wb.construction();
        let mut ids: Vec<_> = ids.into_iter().take(MAX_EDIT_TARGETS + 1).collect();
        if ids.is_empty() || ids.iter().any(|&id| !c.exists(id)) {
            return None;
        }
        let error = (ids.len() > MAX_EDIT_TARGETS).then_some(EditError::SelectionTooLarge);
        ids.sort_unstable();
        ids.dedup();
        let fields = if error.is_some() {
            Vec::new()
        } else {
            Field::ALL
                .iter()
                .copied()
                .filter(|&field| ids.iter().all(|&id| field.supports(c, id)))
                .collect()
        };
        Some(Self {
            tab: Tab::Basic,
            draft: None,
            fields,
            error,
            targets: ids.into_iter().map(|id| c.label(id).to_owned()).collect(),
        })
    }

    pub fn primary(&self) -> &str {
        &self.targets[0]
    }

    pub fn refresh_fields(&mut self, c: &Construction) {
        if self.targets.len() <= MAX_EDIT_TARGETS {
            self.fields = Field::ALL
                .iter()
                .copied()
                .filter(|&field| {
                    self.targets
                        .iter()
                        .all(|label| c.id(label).is_some_and(|id| field.supports(c, id)))
                })
                .collect();
        }
    }

    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

#[derive(Clone, Copy)]
pub enum Input {
    Check,
    Choice(&'static [&'static str]),
    Integer(i32, i32),
    Number(f64, f64),
    Text,
    Color,
}

macro_rules! fields {
    ($($name:ident, $tab:ident, $label:literal, $input:expr;)*) => {
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        pub enum Field { $($name,)* }

        impl Field {
            pub const ALL: &[Self] = &[$(Self::$name,)*];

            pub fn tab(self) -> Tab {
                match self { $(Self::$name => Tab::$tab,)* }
            }

            pub fn label(self) -> &'static str {
                match self { $(Self::$name => $label,)* }
            }

            pub fn input(self) -> Input {
                match self { $(Self::$name => $input,)* }
            }
        }
    };
}

use Input::{Check, Choice, Color, Integer, Number, Text};

// The order follows PropertiesPanelD. Slider, text and algebra controls
// are included in the five inspector tabs.
fields! {
    Caption, Basic, "Podpis", Text;
    Visible, Basic, "Pokaż obiekt", Check;
    ShowLabel, Basic, "Pokaż etykietę", Check;
    LabelMode, Basic, "Treść etykiety", Choice(&["Nazwa", "Nazwa i wartość", "Wartość", "Podpis", "Podpis i wartość"]);
    Animating, Basic, "Animuj", Check;
    Fixed, Basic, "Unieruchom obiekt", Check;
    Auxiliary, Basic, "Obiekt pomocniczy", Check;
    CheckboxFixed, Basic, "Zablokuj położenie pola wyboru", Check;
    ImageBackground, Basic, "Obraz w tle", Check;
    AngleInterval, Basic, "Zakres kąta", Choice(&["0°–360°", "0°–180°", "180°–360°"]);
    RightAngle, Basic, "Wyróżnij kąt prosty", Check;
    Color, Color, "Kolor obiektu", Color;
    FillOpacity, Color, "Krycie wypełnienia (%)", Integer(0, 100);
    TextBackground, Color, "Kolor tła tekstu", Color;
    SliderTrackColor, Color, "Kolor toru suwaka", Color;
    PointSize, Style, "Rozmiar punktu", Integer(1, 9);
    PointStyle, Style, "Kształt punktu", Choice(&["Kropka", "Krzyżyk", "Okrąg", "Plus", "Pełny romb", "Pusty romb", "Trójkąt w górę", "Trójkąt w dół", "Trójkąt w prawo", "Trójkąt w lewo", "Bez obramowania"]);
    Thickness, Style, "Grubość linii", Integer(0, 13);
    LineType, Style, "Styl linii", Choice(&["Ciągła", "Krótkie kreski", "Długie kreski", "Kropki", "Kreski i kropki"]);
    LineOpacity, Style, "Krycie linii (%)", Integer(0, 100);
    AngleRadius, Style, "Rozmiar łuku kąta (px)", Integer(10, 100);
    ImageInterpolate, Style, "Wygładzaj obraz", Check;
    InputLength, Style, "Szerokość pola tekstowego (znaki)", Integer(1, 100);
    SegmentStart, Style, "Początek odcinka", Choice(&["Domyślny", "Kreska", "Strzałka", "Widełki", "Pusty grot", "Pełny grot", "Puste koło", "Pełne koło"]);
    SegmentEnd, Style, "Koniec odcinka", Choice(&["Domyślny", "Kreska", "Strzałka", "Widełki", "Pusty grot", "Pełny grot", "Puste koło", "Pełne koło"]);
    VectorArrow, Style, "Grot wektora", Check;
    SegmentDecoration, Style, "Znaczniki odcinka", Choice(&["Brak", "Jedna kreska", "Dwie kreski", "Trzy kreski", "Jedna strzałka", "Dwie strzałki", "Trzy strzałki"]);
    TextSize, Style, "Rozmiar tekstu (%)", Number(1.0, 10000.0);
    TextBold, Style, "Pogrubienie", Check;
    TextItalic, Style, "Kursywa", Check;
    TextLatex, Style, "Formuła LaTeX", Check;
    SliderKnob, Style, "Rozmiar uchwytu suwaka (px)", Number(1.0, 63.5);
    Absolute, Position, "Położenie względem ekranu", Check;
    Anchor, Position, "Punkt zaczepienia", Text;
    PositionX, Position, "Położenie X", Number(-1e9, 1e9);
    PositionY, Position, "Położenie Y", Number(-1e9, 1e9);
    LabelX, Position, "Przesunięcie etykiety X (px)", Integer(-32768, 32767);
    LabelY, Position, "Przesunięcie etykiety Y (px)", Integer(-32768, 32767);
    Condition, Advanced, "Warunek wyświetlania", Text;
    DynamicSpace, Advanced, "Przestrzeń kolorów", Choice(&["RGB", "HSV", "HSL"]);
    DynamicRed, Advanced, "Składowa koloru 1", Text;
    DynamicGreen, Advanced, "Składowa koloru 2", Text;
    DynamicBlue, Advanced, "Składowa koloru 3", Text;
    DynamicAlpha, Advanced, "Dynamiczne krycie wypełnienia", Text;
    Layer, Advanced, "Warstwa", Integer(0, 9);
    Tooltip, Advanced, "Podpowiedź", Choice(&["Automatycznie", "Włączona", "Wyłączona", "Podpis", "Następna komórka"]);
    Selectable, Advanced, "Zezwalaj na wybieranie", Check;
    LineEquation, Advanced, "Postać równania prostej", Choice(&["Ogólna", "Kierunkowa", "Parametryczna"]);
    ConicEquation, Advanced, "Postać równania krzywej stożkowej", Choice(&["Ogólna", "Kanoniczna", "Parametryczna"]);
    SliderMin, Advanced, "Minimum suwaka", Text;
    SliderMax, Advanced, "Maksimum suwaka", Text;
    SliderWidth, Advanced, "Długość suwaka", Number(0.1, 10000.0);
    SliderHorizontal, Advanced, "Suwak poziomy", Check;
    SliderFixed, Advanced, "Zablokuj położenie suwaka", Check;
    SliderStep, Advanced, "Krok suwaka", Number(0.0, 1e9);
    AnimationSpeed, Advanced, "Prędkość animacji", Number(-100.0, 100.0);
    DecimalPlaces, Advanced, "Miejsca po przecinku", Integer(0, 15);
    SignificantFigures, Advanced, "Cyfry znaczące", Integer(1, 15);
    AnimationMode, Advanced, "Tryb animacji", Choice(&["W obie strony", "Rosnąco", "Malejąco", "Rosnąco raz"]);
}

impl Field {
    pub fn input_for(self, c: &Construction, id: Id) -> Input {
        if self == Self::Thickness && c.has_slider_bounds(id) {
            return Number(-f64::MAX, 127.5);
        }
        self.input()
    }

    pub fn label_for(self, c: &Construction, id: Id) -> &'static str {
        if self == Self::Thickness && c.has_slider_bounds(id) {
            return "Grubość linii (px)";
        }
        self.label()
    }

    pub fn supports(self, c: &Construction, id: Id) -> bool {
        if !c.exists(id) {
            return false;
        }
        use Field::*;
        use Kind as K;
        let kind = c.kind(id);
        let slider = kind == K::Number
            && c.slider_default_slot(id).is_some()
            && matches!(c.def(id), Def::Number(_));
        let checkbox = kind == K::Boolean && matches!(c.def(id), Def::Boolean(_));
        let text = kind == K::Text;
        let control = matches!(kind, K::InputBox | K::Button);
        let angle = kind == K::Angle && c.angle_style(id).is_some_and(|s| s.xml_code() < 3);
        let image = kind == K::Image && c.image(id).is_some();
        let unanchored_text = text
            && c.text_position(id).is_some_and(|position| {
                use geo_kernel::kernel::text::TextPosition;
                matches!(
                    position,
                    TextPosition::Origin | TextPosition::World { .. } | TextPosition::Screen { .. }
                )
            });
        let line = matches!(
            kind,
            K::Segment | K::Line | K::Ray | K::PolyLine | K::Vector
        );
        let fill = matches!(kind, K::Angle | K::Polygon);
        let drawable = slider || checkbox || !matches!(kind, K::Number | K::Boolean);
        let label = drawable && !matches!(kind, K::Image | K::Text | K::Locus | K::PenStroke);
        match self {
            Caption => label,
            Visible | Condition | Selectable | Layer | Tooltip => drawable,
            ShowLabel | LabelMode | LabelX | LabelY => label,
            Fixed => !checkbox && !slider && !control,
            Auxiliary => true,
            Color | DynamicSpace | DynamicRed | DynamicGreen | DynamicBlue => drawable && !image,
            DynamicAlpha => fill,
            FillOpacity => fill || image,
            TextBackground | TextSize | TextBold | TextItalic | TextLatex | Anchor => text,
            CheckboxFixed => checkbox,
            ImageBackground | ImageInterpolate => image,
            AngleInterval => angle && !c.def(id).is_free(),
            RightAngle | AngleRadius => angle,
            PointSize | PointStyle => kind == K::Point,
            Thickness => {
                line || fill
                    || slider
                    || matches!(
                        kind,
                        K::Circle
                            | K::Conic
                            | K::Arc
                            | K::Curve
                            | K::Function
                            | K::ImplicitCurve
                            | K::Locus
                            | K::PenStroke
                    )
            }
            LineType => matches!(kind, K::Line | K::Ray | K::Segment),
            LineOpacity => {
                line || fill
                    || slider
                    || matches!(
                        kind,
                        K::Circle
                            | K::Conic
                            | K::Arc
                            | K::Curve
                            | K::Function
                            | K::ImplicitCurve
                            | K::Locus
                            | K::PenStroke
                    )
            }
            InputLength => kind == K::InputBox,
            DecimalPlaces | SignificantFigures => {
                matches!(c.def(id), Def::InputBox(Some(link)) if c.kind(link).is_number())
            }
            SegmentStart | SegmentEnd | SegmentDecoration => kind == K::Segment,
            VectorArrow => kind == K::Vector,
            Absolute => unanchored_text || slider,
            PositionX | PositionY => slider || checkbox || unanchored_text || kind == K::InputBox,
            LineEquation => kind == K::Line,
            ConicEquation => matches!(kind, K::Circle | K::Conic),
            Animating | SliderTrackColor | SliderKnob | SliderMin | SliderMax | SliderWidth
            | SliderHorizontal | SliderFixed | SliderStep | AnimationSpeed | AnimationMode => {
                slider
            }
        }
    }
}
