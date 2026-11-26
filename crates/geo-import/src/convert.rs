//! Turns the items of a construction into lines of the script language.
//!
//! The items arrive in construction order. Free objects (a point, a vector,
//! a number) become `A = (x, y)` and `r = 3` with the exact value of the
//! element. An expression keeps its definition and a command becomes
//! `label = Name(args)` or `{label, label} = Name(args)`. Each line is
//! executed on a scratch [`Construction`] before it is accepted, so whatever
//! the kernel cannot do is skipped with the kernel's message.

use crate::archive::AssetArchive;
use crate::export::DynamicColor;
use crate::xml::{Document, Element, Item};
use crate::{ImportError, Imported, Mismatch, Skipped};
use geo_kernel::algo::conic_nd::{ConicData, ConicType};
use geo_kernel::algo::general_conic::GeneralConicDef;
use geo_kernel::algo::polygon::PolygonDef;
use geo_kernel::kernel::angle::AngleStyle;
use geo_kernel::kernel::control::{
    encode_scripts, encode_text, visibility_boolean, NumberRounding, SliderLabel,
};
use geo_kernel::kernel::image::{ImageCorner, ImagePlacement, RasterAsset};
use geo_kernel::kernel::text::{TextFont, TextOverrides, TextPosition};
use geo_kernel::kernel::{Def, DrawingStyle};
use geo_kernel::script::equation::Style as EquationStyle;
use geo_kernel::script::saved::Property;
use geo_kernel::script::text::{
    literal as text_literal, push_escaped_char, Properties as TextProperties,
};
use geo_kernel::script::{exec, is_vector_label, multi_output_count};
use geo_kernel::{Construction, Id, Kind, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The longest text a reason may quote from the file, in characters.
const QUOTE_CHARS: usize = 60;

/// The longest reason kept for a skipped object, in characters.
const MAX_REASON_CHARS: usize = 300;

/// The longest plain label or number passed as an ordinary command argument.
const MAX_TOKEN_BYTES: usize = 256;

/// `text`, cut to [`QUOTE_CHARS`] characters (with `...`) so that a reason
/// never grows with the input it talks about.
pub fn quote(text: &str) -> String {
    match text.char_indices().nth(QUOTE_CHARS) {
        Some((end, _)) => format!("{}...", &text[..end]),
        None => text.to_string(),
    }
}

/// Runs the whole conversion.
pub fn convert(doc: Document) -> Result<Imported, ImportError> {
    convert_with_assets(doc, None)
}

pub(crate) fn convert_with_assets(
    doc: Document,
    archive: Option<&AssetArchive<'_>>,
) -> Result<Imported, ImportError> {
    let mut c = Converter {
        function_path_bounds: doc.function_path_bounds,
        ..Converter::default()
    };
    if let Some(rounding) = doc
        .settings
        .significant_figures
        .map(NumberRounding::SignificantFigures)
        .or_else(|| doc.settings.decimals.map(NumberRounding::Decimals))
    {
        let property = Property::NumberRounding(rounding);
        property
            .apply(&mut c.kernel)
            .expect("rounding does not require an object");
        c.out.script.push(property.to_string());
    }
    let mut flags = Flags::default();
    let mut elements = BTreeMap::new();
    let mut polygon_inputs = BTreeMap::new();
    let mut images = Vec::new();
    let mut assets = ImageAssets::default();
    for item in &doc.items {
        match item {
            Item::Expression { .. } => {}
            // A command that makes several objects: every output is a label.
            Item::Command {
                name,
                inputs,
                outputs,
            } => {
                if let ("Intersect", [a, b, selector], [label]) =
                    (name.as_str(), inputs.as_slice(), outputs.as_slice())
                {
                    if a == b && selector == "1" && !label.is_empty() {
                        c.self_circle_first
                            .entry(a.clone())
                            .or_insert_with(|| label.clone());
                    }
                }
                if name == "Polygon" && inputs.len() >= 3 {
                    if let Some(label) = outputs.first() {
                        polygon_inputs.insert(label.as_str(), inputs.as_slice());
                    }
                }
            }
            Item::Element(el) => {
                elements.insert(el.label.as_str(), el.as_ref());
            }
        }
    }
    for item in &doc.items {
        match item {
            Item::Expression { label, exp } => {
                c.expressions.insert(label.clone(), exp.clone());
            }
            Item::Command {
                name,
                inputs,
                outputs,
            } => c.command(name, inputs, outputs, &elements)?,
            Item::Element(el) => {
                flags.count(el);
                if el.kind == "image" && !c.produced.contains(&el.label) {
                    c.claim(&el.label)?;
                    // Load an image in construction order when its anchors are
                    // ready, so subsequent commands can use it as an input.
                    let forward_reference = el.image.as_ref().is_some_and(|image| {
                        image.start_points.iter().any(|(_, corner)| match corner {
                            ImageCorner::Coordinates(_) => false,
                            ImageCorner::Point(source) => c.kernel.id(source).is_none(),
                        })
                    });
                    if forward_reference {
                        images.push(el.as_ref());
                    } else {
                        c.image(el, archive, &mut assets)?;
                    }
                    continue;
                }
                let points = polygon_inputs.get(el.label.as_str()).and_then(|inputs| {
                    inputs
                        .iter()
                        .map(|label| {
                            let point = elements
                                .get(label.as_str())
                                .filter(|el| el.kind == "point")?;
                            let [x, y, z] = coords3(point)?;
                            let (x, y) = (x / z, y / z);
                            (x.is_finite() && y.is_finite()).then_some([x, y])
                        })
                        .collect::<Option<Vec<_>>>()
                });
                c.element(
                    el,
                    points.as_deref(),
                    f64::from(doc.settings.gui_font_size.unwrap_or(12)),
                )?;
                c.restore_slider_early(el);
            }
        }
    }
    // Saved corner references can name points that appear later in the file.
    for el in images {
        c.image(el, archive, &mut assets)?;
    }
    for el in elements.values() {
        if !c.early_bounds.contains(&el.label) {
            c.restore_slider(el);
        }
        c.restore_animation(el, doc.settings.start_animation);
        c.retain_scripts(el);
    }
    for (&label, element) in &elements {
        if let Some(condition) = &element.condition {
            if c.kernel.id(label).is_some() {
                let result = expression_line(condition).and_then(|source| {
                    visibility_boolean(&mut c.kernel, label, &source)?;
                    Ok(source)
                });
                match result {
                    Ok(source) => c.out.conditions.push((label.to_owned(), source)),
                    Err(reason) => c.out.notes.push(format!(
                        "visibility condition for `{}`: {}",
                        quote(label),
                        quote(&reason)
                    )),
                }
            }
        }
    }
    // A text start point may be named later in the construction.
    for el in elements.values() {
        c.apply_text_position(el);
    }
    let mut expressions = StyleExpressions::default();
    for el in elements.values() {
        let Some(id) = c.kernel.id(&el.label) else {
            continue;
        };
        let default_style = c.kernel.drawing_style(id);
        let mut style = DrawingStyle {
            layer: el.layer,
            ..default_style
        };
        // Labels and thickness can be edited on the canvas. The other
        // saved properties still retain their original export representation.
        let mut saved_style = el.object_style.clone();
        style.show_label = el.label_shown.unwrap_or(style.show_label);
        style.line_thickness = el.thickness.map_or(style.line_thickness, |t| t.min(13));
        saved_style.show_label = el.label_shown;
        saved_style.untagged = el.shown.is_none();
        if c.kernel.control(id).is_none() {
            saved_style.caption.clone_from(&el.caption);
        }
        let label = quote(&el.label);
        if let Some(properties) = saved_style.preserved.as_deref_mut() {
            properties.retain(|tag| {
                if tag.validate_import(&c.kernel, id, &mut expressions) {
                    return true;
                }
                c.out.notes.push(format!(
                    "{} of `{label}`: unsupported saved attributes",
                    tag.name
                ));
                false
            });
        }
        saved_style.saved_angle_style = c.kernel.angle_style(id);
        if let Some(name) = el.widget_icon.as_deref().filter(|name| !name.is_empty()) {
            match assets.load(&mut c.kernel, archive, name) {
                Ok(asset) => saved_style.widget_icon = Some((c.kernel.kind(id), asset)),
                Err(reason) => c
                    .out
                    .notes
                    .push(format!("icon of `{label}`: {}", quote(&reason))),
            }
        }
        if let Some(dynamic) = saved_style.dynamic_color.take() {
            match expressions.dynamic_color(&c.kernel, &dynamic) {
                Ok(dynamic) => saved_style.dynamic_color = Some(Box::new(dynamic)),
                Err(reason) => c
                    .out
                    .notes
                    .push(format!("dynamic colour of `{label}`: {}", quote(&reason))),
            }
        }
        if let Some(fill) = saved_style.fill.as_deref_mut() {
            if fill.pattern > crate::export::Fill::MAX_PATTERN {
                c.out
                    .notes
                    .push(format!("the image fill of `{label}` is not kept"));
                fill.pattern = 0;
            }
        }
        if c.kernel.kind(id) == Kind::Sector {
            style.sector_outline_only = saved_style.alpha.is_some_and(|alpha| alpha <= 0.0)
                && saved_style
                    .fill
                    .as_ref()
                    .is_none_or(|fill| fill.pattern == 0);
        }
        // A caption taken from a text that is not there is dropped, as the
        // reference reader drops it.
        if let Some(caption) = saved_style.dynamic_caption.take() {
            if c.kernel
                .id(&caption)
                .is_some_and(|text| c.kernel.kind(text) == Kind::Text)
            {
                saved_style.dynamic_caption = Some(caption);
            } else {
                c.out.notes.push(format!(
                    "dynamic caption of `{label}`: `{}` is not a text",
                    quote(&caption)
                ));
            }
        }
        for tag in &el.reader_stops {
            c.out.notes.push(format!(
                "`<{tag}>` of `{label}` has a number the reference reader cannot read; \
                 it gives up the file there, and what the tag had not yet set is not kept"
            ));
        }
        if let Some(mut animation) = el.slider_animation.clone() {
            for (text, what) in [
                (&mut animation.step, "step"),
                (&mut animation.speed, "speed"),
            ] {
                let Some(source) = text.take() else { continue };
                match expressions.number(&c.kernel, &source) {
                    Ok(source) => *text = Some(source),
                    Err(reason) => c
                        .out
                        .notes
                        .push(format!("animation {what} of `{label}`: {}", quote(&reason))),
                }
            }
            if el.slider.is_some() {
                c.out.slider_animation.insert(el.label.clone(), animation);
            } else {
                saved_style.animation = Some(Box::new(animation));
            }
        }
        let canvas_property = match c.kernel.def(id) {
            Def::Number(_) if el.slider.is_some() => {
                if let Some(mode) = saved_style.label_mode {
                    let property = Property::SliderLabel {
                        label: el.label.clone(),
                        presentation: SliderLabel {
                            mode,
                            caption: el.caption.as_str().into(),
                        },
                    };
                    property
                        .apply(&mut c.kernel)
                        .expect("the slider was just checked");
                    c.out.script.push(property.to_string());
                }
                use geo_kernel::kernel::control::SliderStyle;
                let layout = saved_style.slider.as_deref().filter(|layout| {
                    layout.absolute || layout.width.is_some() || layout.position.is_some()
                });
                let absolute = layout.is_none_or(|layout| layout.absolute);
                let flags = layout.and_then(|layout| layout.flags);
                let step_source = c
                    .out
                    .slider_animation
                    .get(&el.label)
                    .and_then(|animation| animation.step.as_deref());
                let step = step_source
                    .and_then(|step| step.parse::<f64>().ok())
                    .filter(|step| step.is_finite());
                style.line_thickness = el.thickness.unwrap_or(style.line_thickness);
                style.show_label = el.label_shown.unwrap_or(style.show_label);
                let style = SliderStyle {
                    position: layout
                        .and_then(|layout| layout.position)
                        .filter(|[x, y]| x.is_finite() && y.is_finite())
                        .map(|[x, y]| (x, y)),
                    absolute,
                    width: layout
                        .and_then(|layout| layout.width)
                        .filter(|v| v.is_finite())
                        .unwrap_or(if absolute { 200.0 } else { 4.0 }),
                    horizontal: flags.is_none_or(|flags| flags.horizontal),
                    fixed: flags.is_some_and(|flags| flags.fixed)
                        || saved_style.fixed == Some(true),
                    knob_size: saved_style.knob_size.filter(|v| v.is_finite()),
                    step,
                    step_expression: step_source.filter(|_| step.is_none()).map(Into::into),
                };
                (style != SliderStyle::default()).then(|| Property::SliderStyle {
                    label: el.label.clone(),
                    style,
                })
            }
            Def::InputBox(_) => {
                style.show_label = el.label_shown.unwrap_or(style.show_label);
                let mut input = geo_kernel::kernel::control::InputBoxStyle::default();
                input.position = el.label_offset.unwrap_or(input.position);
                input.length = el.input_box_length.unwrap_or(input.length);
                Some(Property::InputBoxStyle {
                    label: el.label.clone(),
                    style: input,
                })
            }
            Def::Boolean(_) => {
                style.show_label = el.label_shown.unwrap_or(style.show_label);
                Some(Property::CheckboxStyle {
                    label: el.label.clone(),
                    style: geo_kernel::kernel::control::CheckboxStyle {
                        position: el.label_offset,
                        fixed: saved_style.checkbox_fixed == Some(true)
                            || saved_style.fixed == Some(true),
                    },
                })
            }
            _ => None,
        };
        if let Some(property) = canvas_property {
            property
                .apply(&mut c.kernel)
                .expect("the control kind was just checked");
            c.out.script.push(property.to_string());
        }
        c.out.object_styles.insert(el.label.clone(), saved_style);
        if let Some([x, y]) = el.label_offset.filter(|&position| {
            position != [0, 0] && !matches!(c.kernel.def(id), Def::Boolean(_) | Def::InputBox(_))
        }) {
            c.out
                .label_offsets
                .insert(el.label.clone(), (f64::from(x), f64::from(y)));
        }
        if style != default_style {
            c.kernel.set_drawing_style(id, style);
            c.out.script.push(
                Property::DrawingStyle {
                    label: el.label.clone(),
                    style,
                }
                .to_string(),
            );
        }
    }
    // A spreadsheet lookup can acquire its value when the XML defines a
    // cell later in the construction. Recheck those results after loading.
    c.out.mismatched.retain(|mismatch| {
        let Some(id) = c.kernel.id(&mismatch.label) else {
            return true;
        };
        if !c
            .kernel
            .reads(id)
            .into_iter()
            .any(|parent| geo_kernel::spreadsheet::Address::parse(c.kernel.label(parent)).is_some())
        {
            return true;
        }
        elements.get(mismatch.label.as_str()).is_none_or(|element| {
            cross_check(
                element,
                &c.kernel,
                id,
                c.expressions.get(&mismatch.label).map(String::as_str),
                None,
            )
            .is_some()
        })
    });
    for (address, input) in doc.spreadsheet_cells {
        if doc.archive_spreadsheet_cells.contains(&address) {
            if let Some(id) = c.kernel.spreadsheet_cell(address) {
                // Older exports could retain the typed literal after a
                // change in Algebra. Keep the saved construction value.
                let source = input.strip_prefix('=').unwrap_or(&input).trim();
                let stale = match c.kernel.def(id) {
                    Def::Number(saved) => source.parse::<f64>().is_ok_and(|typed| typed != saved),
                    Def::Angle(saved) => source.strip_suffix('°').is_some_and(|source| {
                        source
                            .trim()
                            .parse::<f64>()
                            .is_ok_and(|typed| typed.to_radians() != saved)
                    }),
                    Def::FreePoint { x, y } => source
                        .strip_prefix('(')
                        .and_then(|s| s.strip_suffix(')'))
                        .and_then(|s| s.split_once(','))
                        .and_then(|(a, b)| {
                            Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?))
                        })
                        .is_some_and(|typed| typed != (x, y)),
                    _ => false,
                };
                // The main document already reconstructed the cell and
                // its style. The private entry supplies only editor text.
                if !stale {
                    let property = Property::CellInput {
                        label: address.label(),
                        input,
                    };
                    match property.apply(&mut c.kernel) {
                        Ok(()) => c.out.script.push(property.to_string()),
                        Err(reason) => c.out.skipped.push(Skipped {
                            label: address.label(),
                            kind: "cell".into(),
                            reason,
                        }),
                    }
                }
                continue;
            }
        }
        let line = geo_kernel::spreadsheet::edit_line(address, &input);
        match geo_kernel::script::exec(&mut c.kernel, &line) {
            Ok(_) => c.out.script.push(line),
            Err(reason) => c.out.skipped.push(Skipped {
                label: address.label(),
                kind: "cell".into(),
                reason,
            }),
        }
    }
    let mut out = c.out;
    out.settings = doc.settings;
    out.cas_rows = doc.cas_rows;
    out.cas_visible = doc.cas_visible;
    out.spreadsheet_view = doc.spreadsheet_view;
    out.view = doc.view;
    if out.settings.continuous {
        out.notes.push(
            "the construction was saved with continuity on; the kernel does not use it, so \
             the choice between two intersection points can differ from what the file shows"
                .into(),
        );
    }
    if doc.constructions > 1 {
        out.notes.push(format!(
            "the file has {} construction sections; their elements are appended one after \
             the other, as the reader of the format does",
            doc.constructions
        ));
    }
    flags.notes(&mut out.notes);
    Ok(out)
}

/// The evaluation work all style expressions of one document may take
/// together, on import and again on each export: the budget of a single
/// evaluation. Past it the remaining expressions are not kept.
pub const STYLE_EVALUATION_WORK: u64 = geo_kernel::expr::MAX_EVALUATION_WORK;

/// Evaluates the numeric expressions of saved styles (dynamic colour
/// channels, animation steps and speeds) within one budget per document.
pub struct StyleExpressions {
    used: u64,
    budget: u64,
}

impl Default for StyleExpressions {
    fn default() -> Self {
        Self::with_budget(STYLE_EVALUATION_WORK)
    }
}

impl StyleExpressions {
    /// A budget other than [`STYLE_EVALUATION_WORK`], for tests.
    pub fn with_budget(budget: u64) -> Self {
        Self { used: 0, budget }
    }

    /// The work done so far, in the units of the kernel's evaluation budget.
    pub fn work(&self) -> u64 {
        self.used
    }

    /// A saved numeric expression in the kernel's syntax, when the kernel
    /// evaluates it as arithmetic over the construction. Its current value
    /// does not matter: an expression whose inputs are undefined right now
    /// is kept, as the reference reader keeps it.
    pub fn number(&mut self, c: &Construction, source: &str) -> Result<String, String> {
        if self.used >= self.budget {
            return Err("the style expressions of the file exceed the evaluation budget".into());
        }
        let line = expression_line(source)?;
        let before = geo_kernel::expr::work_done();
        let value = c.arithmetic_value(&line);
        // A parsed expression costs at least one unit, so that a file of
        // many cheap ones is bounded as well.
        self.used += (geo_kernel::expr::work_done() - before).max(1);
        // A command the kernel parses without running it is refused by
        // the kernel itself, whatever value it would give.
        value
            .map(|_| line)
            .ok_or_else(|| format!("`{source}` is not a number of the construction"))
    }

    /// The channels of a dynamic colour as expressions the kernel evaluates.
    pub fn dynamic_color(
        &mut self,
        c: &Construction,
        saved: &DynamicColor,
    ) -> Result<DynamicColor, String> {
        Ok(DynamicColor {
            red: self.number(c, &saved.red)?,
            green: self.number(c, &saved.green)?,
            blue: self.number(c, &saved.blue)?,
            alpha: saved
                .alpha
                .as_deref()
                .map(|alpha| self.number(c, alpha))
                .transpose()?,
            space: saved.space,
        })
    }
}

/// Properties of elements that no value depends on and the kernel has no
/// place for; only counted.
#[derive(Default)]
struct Flags {
    fixed: usize,
    auxiliary: usize,
    start_points: usize,
    has_scripts: bool,
}

impl Flags {
    fn count(&mut self, el: &Element) {
        self.fixed += el.fixed as usize;
        self.auxiliary += el.auxiliary as usize;
        self.start_points += el.start_point as usize;
        self.has_scripts |= !el.scripts.is_empty();
    }

    fn notes(&self, notes: &mut Vec<String>) {
        if self.has_scripts {
            notes.push("object scripts are not executed".into());
        }
        for (n, what) in [
            (self.fixed, "fixed (the kernel has no fixed flag)"),
            (self.auxiliary, "auxiliary objects"),
            (
                self.start_points,
                "vectors with a start point (the kernel draws none)",
            ),
        ] {
            if n > 0 {
                notes.push(format!("the file has {n} {what}"));
            }
        }
    }
}

#[derive(Default)]
struct ImageAssets {
    by_name: BTreeMap<Arc<str>, Result<Arc<RasterAsset>, String>>,
    by_offset: BTreeMap<u64, Result<Arc<RasterAsset>, String>>,
    emitted: BTreeSet<Arc<str>>,
}

impl ImageAssets {
    fn load(
        &mut self,
        kernel: &mut Construction,
        archive: Option<&AssetArchive<'_>>,
        name: &str,
    ) -> Result<Arc<RasterAsset>, String> {
        if let Some(result) = self.by_name.get(name) {
            return result.clone();
        }
        let result = (|| {
            let (entry, data) = archive
                .ok_or("the image has no archive asset")?
                .image_entry(name)?;
            self.by_offset
                .entry(entry.local_offset)
                .or_insert_with(|| {
                    kernel.reserve_image_input(entry.size.max(data.len() as u64))?;
                    let bytes =
                        crate::archive::extract_entry(entry, data).map_err(|e| e.to_string())?;
                    kernel
                        .decode_image_asset(name.into(), bytes)?
                        .ok_or_else(|| "the image asset was already registered".into())
                })
                .clone()
        })();
        self.by_name.insert(name.into(), result.clone());
        result
    }
}

#[derive(Default)]
struct Converter {
    out: Imported,
    /// The scratch construction every accepted line was run on.
    kernel: Construction,
    /// Labels of objects made by a command, to tell their elements from free ones.
    produced: BTreeSet<String>,
    /// Every label seen so far, to find duplicates.
    seen: BTreeSet<String>,
    /// Labels of skipped objects and why.
    skipped: BTreeMap<String, String>,
    /// `<expression>` texts waiting for their element.
    expressions: BTreeMap<String, String>,
    function_path_bounds: Option<[f64; 2]>,
    /// Sliders whose expression endpoints were restored right after their
    /// element, before the objects that read them.
    early_bounds: BTreeSet<String>,
    self_circle_first: BTreeMap<String, String>,
    self_circle_swaps: BTreeMap<Id, bool>,
}

/// One script line and the labels it has to create.
struct Step {
    line: String,
    labels: Vec<String>,
    /// The kernel chooses the labels of the objects this line makes (a
    /// polygon and its sides, the vertices of a polygon, its angles); the
    /// labels of the file are matched to them afterwards.
    auto: bool,
}

/// Commands whose outputs the kernel labels itself, as GeoGebra does.
fn labels_by_kernel(name: &str, args: &[String]) -> bool {
    matches!(name, "Polygon" | "PolyLine" | "Vertex" | "InteriorAngles")
        || (name == "Angle" && args.len() == 1)
}

impl Converter {
    fn skip(&mut self, label: &str, kind: &str, mut reason: String) {
        // Whatever produced the reason, it stays short.
        if let Some((end, _)) = reason.char_indices().nth(MAX_REASON_CHARS) {
            reason.truncate(end);
            reason.push_str("...");
        }
        if !label.is_empty() {
            self.skipped.insert(label.to_string(), reason.clone());
        }
        self.out.skipped.push(Skipped {
            label: label.into(),
            kind: kind.into(),
            reason,
        });
    }

    fn claim(&mut self, label: &str) -> Result<(), ImportError> {
        if !label.is_empty() && !self.seen.insert(label.to_string()) {
            return Err(ImportError::DuplicateLabel(label.into()));
        }
        Ok(())
    }

    fn command(
        &mut self,
        name: &str,
        inputs: &[String],
        outputs: &[String],
        elements: &BTreeMap<&str, &Element>,
    ) -> Result<(), ImportError> {
        for label in outputs {
            self.claim(label)?;
            if !label.is_empty() {
                self.produced.insert(label.clone());
            }
        }
        let labelled: Vec<&String> = outputs.iter().filter(|l| !l.is_empty()).collect();
        if labelled.is_empty() {
            // GeoGebra labels an object only once it is defined, and writes
            // no name for one that is not labelled.
            self.skip("", name, "the command has no labelled output".into());
            return Ok(());
        }
        let steps = match labelled.iter().find(|l| !is_label(l)) {
            Some(bad) => Err(format!(
                "`{}` is not a label the script can use",
                quote(bad)
            )),
            None if name == "If" && outputs.len() == 1 => {
                let label = &outputs[0];
                let element = elements.get(label.as_str());
                let result = inputs
                    .iter()
                    .map(|arg| expression_line(arg))
                    .collect::<Result<Vec<_>, _>>()
                    .and_then(|args| {
                        let rhs = format!("If({})", args.join(", "));
                        let Some(el) = element else {
                            return Ok((format!("{label} = {rhs}"), None));
                        };
                        let kind = expression_kind(&el.kind)?;
                        object_expression(label, kind, &rhs).map(|line| (line, Some(kind)))
                    });
                return self.accept_expression(label, name, result, None);
            }
            None if matches!(
                name,
                "Button" | "Checkbox" | "CheckBox" | "Textfield" | "InputBox"
            ) && outputs.len() == 1 =>
            {
                let label = &outputs[0];
                let element = elements.get(label.as_str()).copied();
                let result = match name {
                    "Button" => Ok(control_line(label, "$button", None, element)),
                    "Checkbox" | "CheckBox" => Ok(control_line(
                        label,
                        "$checkBox",
                        Some(boolean_value(element)),
                        element,
                    )),
                    _ => match inputs {
                        [] => Ok(control_line(label, "$inputBox", Some(""), element)),
                        [linked] if is_label(linked) && self.kernel.id(linked).is_some() => {
                            Ok(control_line(label, "$inputBox", Some(linked), element))
                        }
                        [linked] => Err(format!(
                            "the linked object `{}` was not imported",
                            quote(linked)
                        )),
                        _ => Err("an input box can link to one object".into()),
                    },
                };
                result.map(|line| {
                    vec![Step {
                        line,
                        labels: vec![label.clone()],
                        auto: false,
                    }]
                })
            }
            None => self.steps(name, inputs, outputs, elements),
        };
        match steps {
            Ok(steps) => {
                for step in steps {
                    if step.auto {
                        match self.run_auto(&step)? {
                            Ok(unmatched) => {
                                self.out.script.push(step.line);
                                if name == "Polygon" {
                                    // GeoGebra lists the sides before the vertices.
                                    // Restore their inputs before checking saved sides.
                                    for label in &step.labels {
                                        if let Some(el) = elements.get(label.as_str()) {
                                            self.restore_regular_vertex(el)?;
                                        }
                                    }
                                }
                                let reason = format!(
                                    "the kernel does not make an object with this label for `{}`",
                                    step.labels[0]
                                );
                                for label in unmatched {
                                    self.skip(&label, name, reason.clone());
                                }
                            }
                            Err(reason) => {
                                for label in &step.labels {
                                    self.skip(label, name, reason.clone());
                                }
                            }
                        }
                        continue;
                    }
                    match self.run(&step.line, &step.labels)? {
                        Ok(()) => self.out.script.push(step.line),
                        Err(reason) => {
                            for label in &step.labels {
                                self.skip(label, name, reason.clone());
                            }
                        }
                    }
                }
            }
            Err(reason) => {
                for label in labelled {
                    self.skip(label, name, reason.clone());
                }
            }
        }
        Ok(())
    }

    /// Runs a line on the scratch construction. The line is accepted only when
    /// it creates exactly the objects the file has for it. The outer error is
    /// a bug: a line the kernel refused that nevertheless changed it, or
    /// created other objects than it was told to (the forms the importer
    /// writes cannot, and are checked before the run).
    fn run(&mut self, line: &str, expected: &[String]) -> Result<Result<(), String>, ImportError> {
        let before = self.kernel.len();
        let vacancies: BTreeSet<_> = expected
            .iter()
            .filter(|label| {
                self.kernel
                    .id(label)
                    .is_some_and(|id| self.kernel.is_blank_cell(id))
            })
            .collect();
        let result = exec(&mut self.kernel, line);
        // Inline arguments can make anonymous objects in addition to the outputs.
        let created = self
            .kernel
            .ids_from(before)
            .filter(|&id| {
                !self.kernel.label(id).starts_with('\0') && !self.kernel.is_blank_cell(id)
            })
            .count();
        let matches = created + vacancies.len() == expected.len()
            && expected.iter().all(|label| {
                self.kernel.id(label).is_some_and(|id| {
                    !self.kernel.is_blank_cell(id)
                        && (id.index() >= before || vacancies.contains(label))
                })
            });
        match result {
            Ok(_) if matches => Ok(Ok(())),
            Ok(_) => Err(ImportError::Internal(format!(
                "`{line}` created {created} objects, expected {expected:?}"
            ))),
            Err(e) if self.kernel.len() == before => Ok(Err(e)),
            Err(e) => Err(ImportError::Internal(format!(
                "`{line}` failed ({e}) after it had created objects"
            ))),
        }
    }

    /// Runs a line whose objects the kernel labels itself, as GeoGebra does.
    /// Every label the file has for them must be one the kernel made; a file
    /// label with no such object (a side the user renamed) is skipped with its
    /// dependents, and everything else stays in the script. The
    /// outer error is a bug.
    fn run_auto(&mut self, step: &Step) -> Result<Result<Vec<String>, String>, ImportError> {
        let before = self.kernel.len();
        match exec(&mut self.kernel, &step.line) {
            Err(e) if self.kernel.len() == before => return Ok(Err(e)),
            Err(e) => {
                return Err(ImportError::Internal(format!(
                    "`{}` failed ({e}) after it had created objects",
                    step.line
                )))
            }
            Ok(_) => {}
        }
        let made: Vec<String> = self
            .kernel
            .ids_from(before)
            .map(|id| self.kernel.label(id))
            .filter(|label| !label.starts_with('\0'))
            .map(str::to_owned)
            .collect();
        for label in &made {
            // A name the file uses later for another object cannot be told
            // from this one; the duplicate check of `claim` reports it.
            self.seen.insert(label.clone());
        }
        // GeoGebra lists the outputs of `Polygon(A, B, n)` in another order
        // than the kernel creates them, so only the sets are compared.
        let unmatched = step
            .labels
            .iter()
            .filter(|l| !made.contains(l))
            .cloned()
            .collect();
        Ok(Ok(unmatched))
    }

    /// The script lines of a command, or why there are none.
    fn steps(
        &mut self,
        name: &str,
        inputs: &[String],
        outputs: &[String],
        elements: &BTreeMap<&str, &Element>,
    ) -> Result<Vec<Step>, String> {
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!("`{}` is not a command name", quote(name)));
        }
        // A malformed wide command must not cause repeated processing or
        // replication of a large serialized input for each claimed output.
        if inputs
            .iter()
            .map(String::len)
            .sum::<usize>()
            .saturating_mul(outputs.len())
            > crate::xml::MAX_ATTRIBUTE_BYTES
        {
            return Err("command inputs are too long for the number of outputs".into());
        }
        let text_command = match name {
            "Text" | "LaTeX" | "FormulaText" | "LetterToUnicode" | "TextToUnicode"
            | "UnicodeToLetter" | "UnicodeToText" | "Ordinal" | "FractionText"
            | "ScientificText" | "SurdText" | "ContinuedFraction" | "TableText"
            | "VerticalText" | "RotateText" | "ReplaceAll" | "Split" => true,
            "Length" | "Take" | "IndexOf" => inputs.first().is_some_and(|arg| {
                arg.contains('"')
                    || self
                        .kernel
                        .id(arg)
                        .is_some_and(|id| self.kernel.kind(id) == Kind::Text)
            }),
            _ => false,
        };
        let legacy_interval = name == "Function" && matches!(inputs.len(), 3 | 4);
        let mut args = Vec::with_capacity(inputs.len());
        for (index, arg) in inputs.iter().enumerate() {
            let raw = text_command
                || legacy_interval
                || matches!(name, "Curve" | "CurveCartesian" | "Slider")
                || (name == "Angle"
                    && inputs.len() == 1
                    && arg
                        .trim()
                        .strip_suffix('°')
                        .is_some_and(|degrees| degrees.trim().parse::<f64>().is_ok()))
                || (index == 0 && matches!(name, "Locus" | "SlopeField"));
            let arg = expression_line(arg)?;
            if !self.skipped.is_empty() {
                if let Some(token) = geo_kernel::expr::expression_names(&arg)
                    .into_iter()
                    .find(|name| self.skipped.contains_key(*name))
                {
                    return Err(format!("depends on the skipped object `{}`", quote(token)));
                }
            }
            args.push(pass_argument(&arg, raw)?);
        }
        let name = match name {
            "CurveCartesian" => "Curve",
            "Function" if legacy_interval => "$functionInterval",
            _ => name,
        };
        if let ("PointIn", [region], [label]) = (name, args.as_slice(), outputs) {
            if let Some([x, y, z]) = elements.get(label.as_str()).and_then(|el| coords3(el)) {
                let (x, y) = (x / z, y / z);
                if x.is_finite() && y.is_finite() {
                    return Ok(vec![Step {
                        line: format!("{label} = $pointInRegion({region}, {x}, {y})"),
                        labels: vec![label.clone()],
                        auto: false,
                    }]);
                }
            }
        }
        if let ("Point", [path], [label]) = (name, args.as_slice(), outputs) {
            if let Some((el, path_id)) = elements.get(label.as_str()).zip(self.kernel.id(path)) {
                let kind = self.kernel.kind(path_id);
                if kind == Kind::Curve {
                    if let Some(t) = el.curve_parameter {
                        return Ok(vec![Step {
                            line: format!("{label} = $pointOnPath({path}, {t})"),
                            labels: vec![label.clone()],
                            auto: false,
                        }]);
                    }
                }
                if let Some([x, y, z]) = coords3(el).filter(|_| kind != Kind::List) {
                    let (x, y) = (x / z, y / z);
                    if x.is_finite() && y.is_finite() {
                        let bounds = self
                            .function_path_bounds
                            .filter(|_| kind == Kind::Function)
                            .map_or_else(String::new, |[min, max]| format!(", {min}, {max}"));
                        return Ok(vec![Step {
                            line: format!("{label} = $pointOnPath({path}, {x}, {y}{bounds})"),
                            labels: vec![label.clone()],
                            auto: false,
                        }]);
                    }
                }
            }
        }
        if let ("Point", [list]) = (name, args.as_slice()) {
            if self
                .kernel
                .id(list)
                .is_some_and(|id| self.kernel.kind(id) == Kind::List)
            {
                let count = multi_output_count(&self.kernel, &format!("Point({list})"))
                    .and_then(Result::ok)
                    .unwrap_or(0);
                if outputs.len() > count {
                    // The XML handler runs Point with the rows present on
                    // load. Surplus element labels are restored separately.
                    let (active, surplus) = outputs.split_at(count);
                    let mut steps = Vec::new();
                    if active.iter().any(|label| !label.is_empty()) {
                        steps.push(Step {
                            line: format!("{{{}}} = Point({list})", active.join(", ")),
                            labels: active.iter().filter(|l| !l.is_empty()).cloned().collect(),
                            auto: false,
                        });
                    }
                    steps.extend(surplus.iter().filter(|l| !l.is_empty()).map(|label| Step {
                        line: format!("{label} = (?, ?)"),
                        labels: vec![label.clone()],
                        auto: false,
                    }));
                    return Ok(steps);
                }
            }
        }
        if let ("Intersect", [a, b, selector], [_]) = (name, args.as_mut_slice(), outputs) {
            if a == b {
                if let Some(id) = self
                    .kernel
                    .id(a)
                    .filter(|&id| self.kernel.kind(id) == Kind::Circle)
                {
                    if matches!(selector.as_str(), "1" | "2")
                        && self.saved_self_circle_swap(id, a, elements)?
                    {
                        *selector = if selector == "1" { "2" } else { "1" }.into();
                    }
                }
            }
        }
        // GeoGebra saves `Intersect(a, b)` of a line and a conic (or two
        // conics) without an index and lists both points as outputs; the
        // kernel takes the n-th point as `Intersect(a, b, n)`.
        if name == "Intersect"
            && args.len() == 2
            && outputs.len() >= 2
            && !args.iter().any(|arg| {
                self.kernel
                    .id(arg)
                    .is_some_and(|id| self.kernel.kind(id) == Kind::Curve)
            })
        {
            let swapped = self.circle_intersection_swapped(&args, outputs, elements)?;
            return Ok(outputs
                .iter()
                .enumerate()
                .filter(|(_, label)| !label.is_empty())
                .map(|(n, label)| Step {
                    line: format!(
                        "{label} = Intersect({}, {}, {})",
                        args[0],
                        args[1],
                        if swapped { 2 - n } else { n + 1 }
                    ),
                    labels: vec![label.clone()],
                    auto: false,
                })
                .collect());
        }
        let command = match args.as_slice() {
            [a, b, _]
                if name == "Polygon"
                    && outputs
                        .get(1)
                        .and_then(|label| elements.get(label.as_str()))
                        .is_some_and(|el| el.kind == "segment")
                    && [a, b]
                        .into_iter()
                        .filter_map(|label| self.kernel.id(label))
                        .any(|id| {
                            self.kernel.kind(id) == Kind::Point
                                && !self.kernel.value(id).is_defined()
                        })
                    && outputs.iter().any(|label| {
                        elements
                            .get(label.as_str())
                            .is_some_and(|el| el.kind == "point")
                    }) =>
            {
                // A polygon saved with side labels already created them while
                // its endpoints were defined. Allocate those slots on reload.
                "$regularPolygon"
            }
            _ => name,
        };
        let call = format!("{command}({})", args.join(", "));
        let labelled: Vec<String> = outputs.iter().filter(|l| !l.is_empty()).cloned().collect();
        if labels_by_kernel(name, inputs) {
            return Ok(vec![Step {
                line: format!("{} = {call}", labelled[0]),
                labels: labelled,
                auto: true,
            }]);
        }
        let lowered = args.iter().any(|arg| arg.starts_with("$arg("));
        let line = match (!lowered)
            .then(|| multi_output_count(&self.kernel, &call))
            .flatten()
        {
            // A command that makes several objects: every output gets its label.
            Some(_) => format!("{{{}}} = {call}", outputs.join(", ")),
            None if outputs.len() == 1 => format!("{} = {call}", outputs[0]),
            None if lowered => format!("{{{}}} = {call}", outputs.join(", ")),
            None => {
                return Err(format!(
                    "the file lists {} outputs, the kernel makes one object for this command",
                    outputs.len()
                ))
            }
        };
        Ok(vec![Step {
            line,
            labels: labelled,
            auto: false,
        }])
    }

    /// Loading an indexed self-intersection seeds the shared solver from
    /// the saved first position. If that position is stale, the incident
    /// point moves to the second output on the first update.
    fn saved_self_circle_swap(
        &mut self,
        circle: Id,
        name: &str,
        elements: &BTreeMap<&str, &Element>,
    ) -> Result<bool, String> {
        if let Some(&swap) = self.self_circle_swaps.get(&circle) {
            return Ok(swap);
        }
        let Some((label, [x, y, z])) = self
            .self_circle_first
            .get(name)
            .and_then(|label| Some((label, coords3(elements.get(label.as_str())?)?)))
        else {
            return Ok(false);
        };
        let saved = [x / z, y / z];
        if !saved.iter().all(|v| v.is_finite()) {
            return Ok(false);
        }
        let checkpoint = self.kernel.checkpoint();
        let actual = exec(
            &mut self.kernel,
            &format!("{label} = Intersect({name}, {name}, 1)"),
        )
        .ok()
        .flatten()
        .map(|id| self.kernel.value(id));
        self.kernel
            .rollback(checkpoint)
            .map_err(|e| e.to_string())?;
        let swap = matches!(actual, Some(Value::Point { x, y }) if !close(saved[0], x) || !close(saved[1], y));
        self.self_circle_swaps.insert(circle, swap);
        Ok(swap)
    }

    /// A saved pair can keep the permutation from an earlier edit. Select
    /// the opposite slots only when both saved coordinates prove a swap.
    fn circle_intersection_swapped(
        &mut self,
        args: &[String],
        outputs: &[String],
        elements: &BTreeMap<&str, &Element>,
    ) -> Result<bool, String> {
        let [first, second, rest @ ..] = outputs else {
            return Ok(false);
        };
        if rest.iter().any(|label| !label.is_empty()) {
            return Ok(false);
        }
        let [a, b] = args else { return Ok(false) };
        let kind = |label: &str| self.kernel.id(label).map(|id| self.kernel.kind(id));
        if !matches!(
            (kind(a), kind(b)),
            (Some(Kind::Line), Some(Kind::Circle | Kind::Conic))
                | (
                    Some(Kind::Circle | Kind::Conic),
                    Some(Kind::Line | Kind::Circle | Kind::Conic)
                )
        ) {
            return Ok(false);
        }
        let point = |label: &str| {
            let [x, y, z] = coords3(elements.get(label)?)?;
            let xy = [x / z, y / z];
            xy.iter().all(|v| v.is_finite()).then_some(xy)
        };
        let (Some(p), Some(q)) = (point(first), point(second)) else {
            return Ok(false);
        };
        let agrees = |p: [f64; 2], q: [f64; 2]| close(p[0], q[0]) && close(p[1], q[1]);
        if agrees(p, q) {
            return Ok(false);
        }
        let start = self.kernel.checkpoint();
        let mut probe = |label: &str, slot| {
            let id = exec(
                &mut self.kernel,
                &format!("{label} = Intersect({a}, {b}, {slot})"),
            )
            .ok()??;
            match self.kernel.value(id) {
                Value::Point { x, y } => Some([x, y]),
                _ => None,
            }
        };
        let actual_first = probe(first, 1);
        let actual_second = probe(second, 2);
        self.kernel.rollback(start).map_err(|e| e.to_string())?;
        Ok(actual_first.is_some_and(|actual| agrees(actual, q))
            && actual_second.is_some_and(|actual| agrees(actual, p)))
    }

    fn element(
        &mut self,
        el: &Element,
        polygon: Option<&[[f64; 2]]>,
        font_size: f64,
    ) -> Result<(), ImportError> {
        let expression = self.expressions.remove(&el.label);
        if !self.produced.contains(&el.label) {
            self.claim(&el.label)?;
            self.define_element(el, expression.as_deref())?;
        }
        if el.kind == "point" {
            self.restore_closest_circle_angle(el)?;
        }
        self.apply_text_style(el, font_size);
        self.record_element(el, expression.as_deref(), polygon);
        Ok(())
    }

    fn restore_closest_circle_angle(&mut self, el: &Element) -> Result<(), ImportError> {
        let Some(id) = self.kernel.id(&el.label) else {
            return Ok(());
        };
        let Some([x, y, z]) = coords3(el).filter(|&[_, _, z]| z != 0.0) else {
            return Ok(());
        };
        if let Some(angle) = self.kernel.saved_closest_circle_angle(id, x / z, y / z) {
            let property = Property::ClosestCircleAngle {
                label: el.label.clone(),
                angle,
            };
            property
                .apply(&mut self.kernel)
                .map_err(ImportError::Internal)?;
            self.out.script.push(property.to_string());
        }
        Ok(())
    }

    fn restore_regular_vertex(&mut self, el: &Element) -> Result<(), ImportError> {
        let Some(id) = self.kernel.id(&el.label).filter(|_| el.kind == "point") else {
            return Ok(());
        };
        let Def::Polygon(PolygonDef::RegularVertex { a, b, .. }) = self.kernel.def(id) else {
            return Ok(());
        };
        if self.kernel.value(a).is_defined() && self.kernel.value(b).is_defined() {
            return Ok(());
        }
        let Some(coords) = &el.coords else {
            return Ok(());
        };
        let finite = coords3(el)
            .map(|[x, y, z]| (x / z, y / z))
            .filter(|(x, y)| x.is_finite() && y.is_finite());
        let (x, y) = if let Some(point) = finite {
            point
        } else if coords.iter().all(|coord| coord == "NaN") {
            (f64::NAN, f64::NAN)
        } else {
            return Ok(());
        };
        let saved = if x.is_nan() && y.is_nan() {
            Value::Undefined
        } else {
            Value::Point { x, y }
        };
        if self.kernel.value(id) != saved {
            let property = Property::RegularVertex {
                label: el.label.clone(),
                x,
                y,
            };
            property
                .apply(&mut self.kernel)
                .map_err(ImportError::Internal)?;
            self.out.script.push(property.to_string());
        }
        Ok(())
    }

    fn image(
        &mut self,
        el: &Element,
        archive: Option<&AssetArchive<'_>>,
        assets: &mut ImageAssets,
    ) -> Result<(), ImportError> {
        let line = (|| {
            let image = el
                .image
                .as_deref()
                .ok_or("the image has no saved properties")?;
            let name = image
                .file
                .as_deref()
                .filter(|name| !name.is_empty())
                .ok_or("the image has no asset name")?;
            if image.invalid_position {
                return Err("invalid image position".into());
            }
            let mut corners = [None; 4];
            let mut place = |anchor| {
                let (index, corner): &(i32, ImageCorner<String>) = anchor;
                let index = if image.centered { 3 } else { *index as usize };
                if matches!(index, 1 | 2) && corners[0].is_none() {
                    return;
                }
                if let Some(slot) = corners.get_mut(index) {
                    *slot = Some(match corner {
                        ImageCorner::Coordinates(coords) => ImageCorner::Coordinates(*coords),
                        ImageCorner::Point(source) => ImageCorner::Point(source.as_str()),
                    });
                }
            };
            // Inline corner zero is set during XML reading. Other corners
            // and named/expression anchors are installed in their saved order.
            let immediate = |(index, corner): &&(i32, ImageCorner<String>)| {
                *index == 0 && matches!(corner, ImageCorner::Coordinates(_))
            };
            for anchor in image.start_points.iter().filter(immediate) {
                place(anchor);
            }
            for anchor in image
                .start_points
                .iter()
                .filter(|anchor| !immediate(anchor))
            {
                place(anchor);
            }
            let [a, b, d, center] = corners;
            let placement = match (image.anchors_in_screen_coords, image.centered, a) {
                (true, true, _) | (true, false, None) => {
                    ImagePlacement::Screen(image.screen.unwrap_or_default())
                }
                _ => ImagePlacement::World {
                    corners: if image.centered {
                        [center, None, None]
                    } else {
                        [a, b, d]
                    },
                    centered: image.centered,
                    screen_coords: image.anchors_in_screen_coords,
                },
            };
            assets.load(&mut self.kernel, archive, name)?;
            let rhs = geo_kernel::script::image::definition(name, placement, image.style);
            Ok(format!("{} = {rhs}", el.label))
        })();
        match line {
            Ok(line) => match self.run(&line, std::slice::from_ref(&el.label))? {
                Ok(()) => {
                    let id = self.kernel.id(&el.label).expect("accepted image");
                    let image = self.kernel.image(id).expect("accepted raster");
                    if assets.emitted.insert(image.name.clone()) {
                        self.out.image_assets.push(Property::ImageAsset {
                            name: image.name.clone(),
                            asset: image.asset.clone(),
                        });
                    }
                    self.out.script.push(line);
                    self.record_element(el, None, None);
                }
                Err(e) => self.skip(&el.label, &el.kind, e),
            },
            Err(e) => self.skip(&el.label, &el.kind, e),
        }
        Ok(())
    }

    fn define_element(
        &mut self,
        el: &Element,
        expression: Option<&str>,
    ) -> Result<(), ImportError> {
        if let Some(exp) = expression.filter(|exp| !has_saved_literal(el, exp)) {
            let result = expression_kind(&el.kind).and_then(|kind| {
                object_expression(&el.label, kind, exp).map(|line| (line, Some(kind)))
            });
            return self.accept_expression(&el.label, &el.kind, result, Some((el, exp)));
        }
        match self.free_object(el)? {
            Ok(line) => match self.run(&line, std::slice::from_ref(&el.label))? {
                Ok(()) => self.out.script.push(line),
                Err(e) => self.skip(&el.label, &el.kind, e),
            },
            Err(reason) => self.skip(&el.label, &el.kind, reason),
        }
        Ok(())
    }

    fn record_element(
        &mut self,
        el: &Element,
        expression: Option<&str>,
        polygon: Option<&[[f64; 2]]>,
    ) {
        if let Some(id) = self.kernel.id(&el.label) {
            if let Some(form) = el
                .equation_form
                .as_ref()
                .filter(|form| self.kernel.equation_form(id) != Some(*form))
            {
                let line = EquationStyle {
                    label: &el.label,
                    form: form.clone(),
                }
                .to_string();
                match exec(&mut self.kernel, &line) {
                    Ok(_) => self.out.script.push(line),
                    Err(e) => self
                        .out
                        .notes
                        .push(format!("equation style for `{}`: {e}", quote(&el.label))),
                }
            }
            if let Some(current) = self.kernel.angle_style(id) {
                let initial =
                    if expression.is_none() && matches!(self.kernel.def(id), Def::Angle(_)) {
                        AngleStyle::default()
                    } else {
                        current
                    };
                let style = el
                    .object_style
                    .preserved
                    .as_deref()
                    .map_or(initial, |properties| properties.angle_style(initial));
                if style != current {
                    let line = format!("$angleStyle({}, {})", el.label, style.xml_code());
                    match exec(&mut self.kernel, &line) {
                        Ok(_) => self.out.script.push(line),
                        Err(e) => self
                            .out
                            .notes
                            .push(format!("angle style for `{}`: {e}", quote(&el.label))),
                    }
                }
            }
            // A restored self-intersection can exchange its output slots
            // on load. Its live coordinates then differ from the saved ones.
            let restored_self_intersection = matches!(
                self.kernel.def(id),
                Def::IntersectCircles { c1, c2, .. }
                    if c1 == c2 && self.self_circle_swaps.get(&c1) == Some(&true)
            );
            if !restored_self_intersection {
                if let Some((saved, computed)) =
                    cross_check(el, &self.kernel, id, expression, polygon)
                {
                    self.out.mismatched.push(Mismatch {
                        label: el.label.clone(),
                        saved,
                        computed,
                    });
                }
            }
            // Without a `<show>` tag the object stays visible in the editor.
            if el.shown == Some(false) {
                self.out.hidden.push(el.label.clone());
            }
        }
    }

    /// The reference's reader sets a slider's endpoints while it reads the
    /// slider's element, so an object that reads the slider later (an
    /// optimizer checking its bounds) already sees them. Expression
    /// endpoints whose objects exist at this point are restored now; the
    /// others wait for the end of the file.
    fn restore_slider_early(&mut self, el: &Element) {
        let Some([Some(min), Some(max)]) = &el.slider else {
            return;
        };
        if el.kind != "numeric" || literal_slider_bounds(el).is_some() {
            return;
        }
        let (Ok(min), Ok(max)) = (expression_line(min), expression_line(max)) else {
            return;
        };
        if self.kernel.id(&el.label).is_none() {
            return;
        }
        let property = Property::SliderBounds {
            label: el.label.clone(),
            bounds: [Some(min), Some(max)],
        };
        let checkpoint = self.kernel.checkpoint();
        let (restored, errors) = property.restore(&mut self.kernel);
        match restored {
            Some(property) if errors.is_empty() => {
                self.out.script.push(property.to_string());
                self.early_bounds.insert(el.label.clone());
            }
            _ => {
                // an endpoint object comes later in the file
                let _ = self.kernel.rollback(checkpoint);
            }
        }
    }

    /// A slider saved as playing plays again, and moves when the file
    /// started the timer. A step or speed that is no plain number is not
    /// kept for the animation.
    fn restore_animation(&mut self, el: &Element, running: bool) {
        let Some(animation) = el.slider_animation.as_ref().filter(|a| a.playing) else {
            return;
        };
        if el.slider.is_none() || self.kernel.id(&el.label).is_none() {
            return;
        }
        let plain = |source: &Option<String>| -> Result<Option<f64>, ()> {
            source
                .as_deref()
                .map(|s| {
                    s.trim()
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .ok_or(())
                })
                .transpose()
        };
        let label = quote(&el.label);
        let (Ok(step), Ok(speed)) = (plain(&animation.step), plain(&animation.speed)) else {
            self.out.notes.push(format!(
                "slider `{label}` does not play: its saved step or speed is an expression, kept for the file only"
            ));
            return;
        };
        let property = Property::SliderAnimation {
            label: el.label.clone(),
            step,
            speed: speed.unwrap_or(1.0),
            mode: animation.mode,
            running,
        };
        match property.apply(&mut self.kernel) {
            Ok(()) => self.out.script.push(property.to_string()),
            Err(reason) => self
                .out
                .notes
                .push(format!("animation of `{label}`: {}", quote(&reason))),
        }
    }

    fn restore_slider(&mut self, el: &Element) {
        let Some(saved) = &el.slider else { return };
        if literal_slider_bounds(el).is_some_and(|[min, max]| {
            self.kernel
                .id(&el.label)
                .and_then(|id| self.kernel.slider_interval(id))
                == Some((min, max))
        }) {
            return;
        }

        let label = quote(&el.label);
        let mut bounds = [None, None];
        for ((bound, saved), endpoint) in bounds.iter_mut().zip(saved).zip(["minimum", "maximum"]) {
            if let Some(source) = saved {
                match expression_line(source) {
                    Ok(source) => *bound = Some(source),
                    Err(reason) => self
                        .out
                        .notes
                        .push(format!("slider `{label}` {endpoint}: {}", quote(&reason))),
                }
            }
        }
        if bounds == [None, None] {
            if *saved == [None, None] {
                self.out
                    .notes
                    .push(format!("slider `{label}` has no saved endpoints"));
            }
            return;
        }
        let property = Property::SliderBounds {
            label: el.label.clone(),
            bounds,
        };
        let (restored, errors) = property.restore(&mut self.kernel);
        if let Some(property) = restored {
            self.out.script.push(property.to_string());
        }
        for reason in errors {
            self.out
                .notes
                .push(format!("slider `{label}` bounds: {}", quote(&reason)));
        }
    }

    fn retain_scripts(&mut self, el: &Element) {
        if el.scripts.is_empty()
            || self
                .kernel
                .id(&el.label)
                .is_some_and(|id| self.kernel.control(id).is_some())
        {
            return;
        }
        self.out.script.push(
            Property::ObjectScripts {
                label: el.label.clone(),
                scripts: el.scripts.clone(),
            }
            .to_string(),
        );
    }

    fn apply_text_style(&mut self, el: &Element, font_size: f64) {
        let Some(style) = el
            .text
            .as_deref()
            .filter(|_| self.kernel.id(&el.label).is_some())
        else {
            return;
        };
        self.apply_text_overrides(
            &el.label,
            TextOverrides {
                latex: style.latex,
                font: style.font.as_ref().map(|font| TextFont {
                    size: font.multiplier.unwrap_or_else(|| {
                        font.legacy_size.map_or(1.0, |size| {
                            (font_size + f64::from(size)).max(4.0) / font_size
                        })
                    }),
                    serif: font.serif,
                    bold: font.style & 1 != 0,
                    italic: font.style & 2 != 0,
                }),
                offset: style.offset,
                ..TextOverrides::default()
            },
        );
    }

    fn apply_text_position(&mut self, el: &Element) {
        let Some((saved, id)) = el
            .text
            .as_deref()
            .and_then(|style| style.position.as_ref())
            .zip(self.kernel.id(&el.label))
            .filter(|&(_, id)| self.kernel.kind(id) == Kind::Text)
        else {
            return;
        };
        let from_command = self.produced.contains(&el.label)
            && self.kernel.text_position(id) != Some(TextPosition::Origin);
        let position = match saved {
            TextPosition::Point(label) | TextPosition::ScreenPoint(label) => {
                if !self
                    .kernel
                    .id(label)
                    .is_some_and(|p| self.kernel.kind(p) == Kind::Point)
                {
                    self.out.notes.push(format!(
                        "text `{}` has an unavailable start point `{}`",
                        quote(&el.label),
                        quote(label)
                    ));
                    None
                } else if matches!(saved, TextPosition::ScreenPoint(_)) {
                    Some(TextPosition::ScreenPoint(label.as_str()))
                } else {
                    Some(TextPosition::Point(label.as_str()))
                }
            }
            TextPosition::World { x, y } if !from_command => {
                Some(TextPosition::World { x: *x, y: *y })
            }
            TextPosition::Screen { x, y } => Some(TextPosition::Screen { x: *x, y: *y }),
            TextPosition::Undefined if !from_command => Some(TextPosition::Undefined),
            _ => None,
        };
        self.apply_text_overrides(
            &el.label,
            TextOverrides {
                position,
                ..TextOverrides::default()
            },
        );
    }

    fn apply_text_overrides(&mut self, label: &str, overrides: TextOverrides<&str>) {
        if overrides == TextOverrides::default() {
            return;
        }
        let properties = TextProperties { label, overrides };
        match properties.apply(&mut self.kernel) {
            Ok(()) => self.out.script.push(properties.to_string()),
            Err(e) => self
                .out
                .notes
                .push(format!("text properties for `{}`: {e}", quote(label))),
        }
    }

    /// Roll back any objects that script dispatch creates while trying a
    /// definition that doesn't yield exactly the saved object.
    fn accept_expression(
        &mut self,
        label: &str,
        saved_type: &str,
        line: Result<(String, Option<Kind>), String>,
        expression: Option<(&Element, &str)>,
    ) -> Result<(), ImportError> {
        let before = self.kernel.len();
        let vacancy = self
            .kernel
            .id(label)
            .filter(|&id| self.kernel.is_blank_cell(id));
        let mut checkpoint = Some(self.kernel.checkpoint());
        let result = line.and_then(|(line, kind)| {
            let rhs = line.split_once('=').map_or("", |(_, rhs)| rhs.trim_start());
            let rhs = rhs
                .strip_prefix("$point")
                .or_else(|| rhs.strip_prefix("$vector"))
                .unwrap_or(rhs);
            if !self.skipped.is_empty() {
                if let Some(name) = geo_kernel::expr::expression_names(rhs)
                    .into_iter()
                    .find(|name| self.skipped.contains_key(*name))
                {
                    return Err(format!("depends on the skipped object `{}`", quote(name)));
                }
            }
            let id = exec(&mut self.kernel, &line).map_err(|error| {
                for prefix in ["unknown object `", "unknown command `"] {
                    if let Some(name) = error.strip_prefix(prefix).and_then(|e| e.strip_suffix('`'))
                    {
                        if self.skipped.contains_key(name) {
                            return format!("depends on the skipped object `{}`", quote(name));
                        }
                    }
                }
                format!("expression: {error}")
            })?;
            let Some(id) = id.filter(|&id| {
                self.kernel.label(id) == label
                    && (id.index() >= before || vacancy == Some(id))
                    && !self.kernel.is_blank_cell(id)
                    && self.kernel.ids_from(before).all(|made| {
                        made == id
                            || self.kernel.is_blank_cell(made)
                            || kind == Some(Kind::Text) && self.kernel.label(made).starts_with('\0')
                    })
            }) else {
                return Err(
                    "the expression must create exactly one object with the saved label".into(),
                );
            };
            let actual = self.kernel.kind(id);
            if let Some(kind) = kind.filter(|&kind| kind != actual) {
                return Err(format!(
                    "the expression gives a {actual}, expected a {kind}"
                ));
            }
            if let Some((el, exp)) =
                expression.filter(|_| vacancy.is_none() && self.kernel.reads(id).is_empty())
            {
                if let Some(literal) = exact_literal(el) {
                    let line = format!("{label} = $saved({literal}, {})", expression_line(exp)?);
                    self.kernel
                        .rollback(
                            checkpoint
                                .take()
                                .expect("the original expression is still installed"),
                        )
                        .map_err(|e| e.to_string())?;
                    checkpoint = Some(self.kernel.checkpoint());
                    exec(&mut self.kernel, &line)?;
                    return Ok(line);
                }
            }
            Ok(line)
        });
        match result {
            Ok(line) => self.out.script.push(line),
            Err(reason) => {
                if self.kernel.len() != before {
                    self.kernel
                        .rollback(
                            checkpoint
                                .take()
                                .expect("the original expression is still installed"),
                        )
                        .map_err(|e| ImportError::Internal(e.to_string()))?;
                }
                self.skip(label, saved_type, reason);
            }
        }
        Ok(())
    }

    /// The script line of an element that no command made, or why there is
    /// none. The outer `Result` is for a file that is damaged.
    fn free_object(&self, el: &Element) -> Result<Result<String, String>, ImportError> {
        let label = el.label.as_str();
        if !is_label(label) {
            return Ok(Err(format!(
                "`{}` is not a label the script can use",
                quote(label)
            )));
        }
        let number = |text: &str| -> Result<f64, ImportError> {
            text.trim()
                .parse::<f64>()
                .map_err(|_| ImportError::BadNumber {
                    label: label.into(),
                    text: text.into(),
                })
        };
        // The line is written with `{}`, which prints the shortest text that
        // reads back as the same f64.
        Ok(match el.kind.as_str() {
            "point" | "vector" => {
                let Some([x, y, z]) = &el.coords else {
                    return Ok(Err("the element has no coordinates".into()));
                };
                let (x, y, z) = (number(x)?, number(y)?, number(z)?);
                if el.kind == "point" {
                    if ![x, y, z].into_iter().all(f64::is_finite) {
                        return Ok(Err("the coordinates are not finite".into()));
                    }
                    if z != 1.0 {
                        return Ok(Ok(format!("{label} = $homogeneousPoint({x}, {y}, {z})")));
                    }
                    Ok(if is_vector_label(label) {
                        format!("{label} = $point({x}, {y})")
                    } else {
                        format!("{label} = ({x}, {y})")
                    })
                } else {
                    if !(x.is_finite() && y.is_finite()) {
                        return Ok(Err("the coordinates are not finite".into()));
                    }
                    Ok(if is_vector_label(label) {
                        format!("{label} = ({x}, {y})")
                    } else {
                        format!("{label} = $vector({x}, {y})")
                    })
                }
            }
            "numeric" => {
                let Some(text) = &el.value else {
                    return Ok(Err("the element has no value".into()));
                };
                let v = number(text)?;
                if v.is_finite() {
                    if let Some([min, max]) = literal_slider_bounds(el) {
                        Ok(format!("{label} = $slider({v}, {min}, {max})"))
                    } else {
                        Ok(format!("{label} = {v}"))
                    }
                } else {
                    Err("the value is not finite".into())
                }
            }
            "penstroke" => {
                use geo_kernel::algo::stroke::{
                    parse_saved_coords, saved_definition, smooth_saved_vertices, StrokeData,
                };
                // Bezier points replace whatever the vertices made; the
                // vertices apply to a stroke without points
                let bezier = el
                    .stroke_coords
                    .as_deref()
                    .map(parse_saved_coords)
                    .transpose()
                    .map(|numbers| numbers.map(|n| StrokeData::from_triplets(&n)));
                match bezier {
                    Err(reason) => Err(reason),
                    Ok(Some(data)) if !data.points.is_empty() => {
                        Ok(format!("{label} = {}", saved_definition(&data)))
                    }
                    Ok(_) => match &el.legacy_stroke {
                        None => Ok(format!("{label} = PenStroke()")),
                        Some(coords) => match parse_saved_coords(coords) {
                            Err(reason) => Err(reason),
                            // The reference smooths the vertices in the view
                            // its application has while it reads, which the
                            // file does not determine: only pieces of fewer
                            // than three points, which no view changes, are
                            // restored.
                            Ok(numbers) => smooth_saved_vertices(&numbers, None)
                                .map(|data| format!("{label} = {}", saved_definition(&data)))
                                .ok_or_else(|| {
                                    "the stroke vertices are smoothed in the view the reader \
                                     has, which the file does not determine"
                                        .into()
                                }),
                        },
                    },
                }
            }
            "implicitpoly" => Ok(format!(
                "$implicitCoefficients({label}, {})",
                el.implicit_coefficients.as_deref().unwrap_or("[]")
            )),
            "angle" => match free_angle_raw(el) {
                Some(radians) if radians.is_finite() => Ok(format!(
                    "{label} = $saved({radians} rad, {}°)",
                    radians.to_degrees()
                )),
                _ => Err("the angle has no finite value".into()),
            },
            "boolean" => Ok(control_line(
                label,
                "$checkBox",
                Some(boolean_value(Some(el))),
                Some(el),
            )),
            "button" => Ok(control_line(label, "$button", None, Some(el))),
            "textfield" => Ok(control_line(label, "$inputBox", Some(""), Some(el))),
            "text" => el
                .value
                .as_ref()
                .map(|s| format!("{label} = {}", text_literal(s)))
                .ok_or_else(|| "the text has no saved expression or value".into()),
            "conic" => Err("a conic that no command made: the kernel only has circles".into()),
            other => Err(unsupported_element(other)),
        })
    }
}

fn boolean_value(element: Option<&Element>) -> &'static str {
    if element.and_then(|el| el.value.as_deref()) == Some("false") {
        "false"
    } else {
        "true"
    }
}

fn control_line(label: &str, form: &str, value: Option<&str>, element: Option<&Element>) -> String {
    let caption = element.map_or("", |el| el.caption.as_str());
    let mut args = vec![encode_text(caption)];
    if let Some(value) = value {
        args.insert(0, value.to_string());
    }
    if let Some(el) = element {
        args.extend(encode_scripts(&el.scripts));
    }
    format!("{label} = {form}({})", args.join(", "))
}

fn unsupported_element(kind: &str) -> String {
    let reason = match kind {
        "audio" | "video" | "embed" => "out of scope: media playback and embedded applications",
        "implicitpoly" => "there is no implicit curve kind",
        "turtle" => "there is no corresponding turtle kind",
        _ => return format!("the element type `{}` is not supported", quote(kind)),
    };
    format!("element type `{kind}`: {reason}")
}

fn literal_slider_bounds(el: &Element) -> Option<[f64; 2]> {
    let [min, max] = el.slider.as_ref()?;
    Some([num(min.as_deref()?)?, num(max.as_deref()?)?])
}

fn expression_kind(saved_type: &str) -> Result<Kind, String> {
    Ok(match saved_type {
        "point" => Kind::Point,
        "vector" => Kind::Vector,
        "numeric" => Kind::Number,
        "button" => Kind::Button,
        "textfield" => Kind::InputBox,
        "image" => Kind::Image,
        "piechart" => Kind::PieChart,
        "angle" => Kind::Angle,
        "function" => Kind::Function,
        "functionnvar" => Kind::FunctionNVar,
        "boolean" => Kind::Boolean,
        "list" => Kind::List,
        "text" => Kind::Text,
        "line" => Kind::Line,
        "conic" => Kind::Conic,
        "implicitpoly" => Kind::ImplicitCurve,
        other => return Err(unsupported_element(other)),
    })
}

/// Literal objects use the full precision of the saved value.
fn has_saved_literal(el: &Element, exp: &str) -> bool {
    if el.kind == "penstroke" {
        // `AlgoLocusStroke` saves `PenStroke[]`; the points are in the
        // element
        return matches!(exp.trim(), "PenStroke[]" | "PenStroke()");
    }
    if el.kind == "boolean" && el.value.is_some() {
        return matches!(exp.trim(), "true" | "false" | "True" | "False");
    }
    let has_value = match el.kind.as_str() {
        "point" | "vector" => el.coords.is_some(),
        "numeric" => el.value.is_some(),
        _ => false,
    };
    has_value && is_literal(exp)
}

fn free_angle_raw(el: &Element) -> Option<f64> {
    let raw = num(el.value.as_deref()?)?;
    Some(match el.object_style.preserved.as_deref() {
        Some(properties) => properties.free_angle_raw(raw, el.angle_value_position),
        None => AngleStyle::Anticlockwise.value(raw),
    })
}

fn exact_literal(el: &Element) -> Option<String> {
    match el.kind.as_str() {
        "point" | "vector" => {
            let [mut x, mut y, z] = coords3(el)?;
            let form = if el.kind == "point" {
                x /= z;
                y /= z;
                "$point"
            } else {
                "$vector"
            };
            (x.is_finite() && y.is_finite()).then(|| format!("{form}({x}, {y})"))
        }
        "numeric" => Some(num(el.value.as_deref()?)?.to_string()),
        "angle" => Some(format!("{} rad", num(el.value.as_deref()?)?)),
        _ => None,
    }
}

fn object_expression(label: &str, kind: Kind, exp: &str) -> Result<String, String> {
    if !is_label(label) {
        return Err(format!(
            "`{}` is not a label the script can use",
            quote(label)
        ));
    }
    if matches!(kind, Kind::Function | Kind::FunctionNVar) {
        // The XML writer includes the signature of a named function in `exp`.
        let signature = exp.split_once('=').and_then(|(lhs, rhs)| {
            let (name, parameter) = lhs.trim_end().strip_suffix(')')?.split_once('(')?;
            (!rhs.starts_with('=')).then_some((name.trim(), parameter.trim(), rhs.trim()))
        });
        if kind == Kind::FunctionNVar {
            let Some((name, parameters, body)) = signature else {
                return Err("a function of multiple variables needs a signature".into());
            };
            if name != label {
                return Err("the function signature names a different object".into());
            }
            return Ok(format!(
                "{label}({parameters}) = {}",
                expression_line(body)?
            ));
        }
        let (parameter, body) = if let Some((name, parameter, body)) = signature {
            if name != label {
                return Err(format!(
                    "the function signature names `{}`, expected `{}`",
                    quote(name),
                    quote(label)
                ));
            }
            if parameter.contains(',') {
                return Err("a function with multiple variables is unsupported".into());
            }
            if !matches!(parameter, "x" | "y") {
                return Err(format!(
                    "the function parameter `{}` is unsupported",
                    quote(parameter)
                ));
            }
            (parameter, body)
        } else {
            ("x", exp)
        };
        return Ok(format!("{label}({parameter}) = {}", expression_line(body)?));
    }
    let exp = if kind == Kind::ImplicitCurve {
        exp.strip_prefix(label)
            .and_then(|s| s.strip_prefix(":="))
            .unwrap_or(exp)
    } else {
        exp
    };
    let rhs = expression_line(exp)?;
    Ok(match kind {
        Kind::Line | Kind::Conic | Kind::ImplicitCurve => format!("{label}: {rhs}"),
        Kind::Point if is_vector_label(label) => format!("{label} = $point({rhs})"),
        Kind::Vector if !is_vector_label(label) => format!("{label} = $vector({rhs})"),
        _ => format!("{label} = {rhs}"),
    })
}

/// Convert saved command brackets to parentheses. The kernel already reads
/// the saved spellings of comparisons, logical operators, powers and degrees.
pub fn expression_line(exp: &str) -> Result<String, String> {
    let mut out = String::with_capacity(exp.len());
    let mut closers = Vec::new();
    let mut quoted = false;
    for ch in exp.trim().chars() {
        if quoted {
            if ch == '"' {
                quoted = false;
                out.push(ch);
            } else {
                push_escaped_char(&mut out, ch);
            }
            continue;
        }
        let normalized = match ch {
            '#' => {
                return Err("a script comment marker `#` in an expression is unsupported".into())
            }
            '"' => {
                quoted = true;
                ch
            }
            '[' => {
                let end = out.trim_end().len();
                let name = out[..end]
                    .rsplit(|ch: char| !ch.is_ascii_alphanumeric())
                    .next()
                    .unwrap_or("");
                if !name.starts_with(|ch: char| ch.is_ascii_uppercase()) {
                    return Err("a square bracket must follow a command name".into());
                }
                out.truncate(end);
                closers.push(']');
                '('
            }
            '(' | '{' => {
                closers.push(if ch == '(' { ')' } else { '}' });
                ch
            }
            ')' | ']' | '}' => {
                let expected = closers.pop();
                if expected != Some(ch) {
                    if ch == ']' || expected == Some(']') {
                        return Err("mismatched command square brackets in expression".into());
                    }
                    return Err(format!("unexpected `{ch}` in expression"));
                }
                if ch == ']' {
                    ')'
                } else {
                    ch
                }
            }
            '·' => '*',
            '\r' | '\n' => ' ',
            _ => ch,
        };
        out.push(normalized);
    }
    if quoted {
        return Err("unclosed text literal in expression".into());
    }
    if let Some(closer) = closers.pop() {
        return Err(format!("missing `{closer}` in expression"));
    }
    Ok(out)
}

/// Labels and numbers keep the command's ordinary overload resolution.
/// The remaining arguments can create anonymous expression objects on demand.
fn pass_argument(arg: &str, raw: bool) -> Result<String, String> {
    if arg.contains('\0') || arg.is_empty() {
        return Err("invalid command input".into());
    }
    if raw || (arg.len() <= MAX_TOKEN_BYTES && (is_label(arg) || num(arg).is_some())) {
        Ok(arg.to_owned())
    } else {
        Ok(format!("$arg({arg})"))
    }
}

/// A number written by GeoGebra: `3`, `-0.5`, `1.0E-5`, or a point `(x, y)`.
fn is_literal(s: &str) -> bool {
    let number = |t: &str| t.trim().parse::<f64>().is_ok_and(f64::is_finite);
    let s = s.trim();
    match s.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
        Some(inner) => matches!(inner.split_once(','), Some((x, y)) if number(x) && number(y)),
        None => number(s),
    }
}

/// An expression with no object in it, only numbers and operators:
/// digits, `.`, `,`, `+ - * / ^`, parentheses, white space, and an exponent
/// `E` or `e` right after a digit or `.`. The importer uses parsed dependencies
/// to recognize independent objects, including expressions with named constants.
pub fn is_constant_expression(s: &str) -> bool {
    let mut previous = ' ';
    let mut digits = false;
    for ch in s.chars() {
        let ok = match ch {
            '0'..='9' => {
                digits = true;
                true
            }
            '.' | ',' | '+' | '-' | '*' | '/' | '^' | '(' | ')' => true,
            c if c.is_whitespace() => true,
            'E' | 'e' => previous.is_ascii_digit() || previous == '.',
            _ => false,
        };
        if !ok {
            return false;
        }
        previous = ch;
    }
    digits
}

/// A label: letters, digits, `_`, `'` and an index `_{...}` whose inside has
/// letters, digits, `_` and `'` (the same rule as the kernel's output labels).
fn is_label(s: &str) -> bool {
    let (base, index) = match s.strip_suffix('}').and_then(|t| t.split_once("_{")) {
        Some((base, inner)) => (base, Some(inner)),
        None => (s, None),
    };
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    let ok_index = index.is_none_or(|d| !d.is_empty() && d.chars().all(word));
    let mut chars = base.chars();
    let first = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_');
    ok_index && first && chars.all(word)
}

const TOLERANCE: f64 = 1e-9;

/// `|p - q| <= 1e-9 * max(1, |p|, |q|)`, the comparison of the golden files.
fn close(p: f64, q: f64) -> bool {
    p.is_finite() && q.is_finite() && (p - q).abs() <= TOLERANCE * 1.0_f64.max(p.abs()).max(q.abs())
}

fn num(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The infinity a saved number holds, as Java writes it.
fn saved_infinity(text: &str) -> Option<f64> {
    match text.trim() {
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        _ => None,
    }
}

/// Three numbers of `<coords>`, all finite.
fn coords3(el: &Element) -> Option<[f64; 3]> {
    let [x, y, z] = el.coords.as_ref()?;
    Some([num(x)?, num(y)?, num(z)?])
}

/// Whether the lines `(a, b, c)` and `(sa, sb, sc)` are the same up to a
/// positive factor (the orientation of a line is part of its value).
fn same_line(line: [f64; 3], saved: [f64; 3]) -> bool {
    let (n, sn) = (line[0].hypot(line[1]), saved[0].hypot(saved[1]));
    n > 0.0 && sn > 0.0 && (0..3).all(|i| close(line[i] / n, saved[i] / sn))
}

fn saved_text(el: &Element) -> String {
    let mut parts = Vec::new();
    if let Some([x, y, z]) = &el.coords {
        parts.push(format!("coords ({x}, {y}, {z})"));
    }
    if let Some(m) = &el.matrix {
        parts.push(format!("matrix [{}]", m.join(", ")));
    }
    if let Some(v) = &el.value {
        parts.push(format!("value {v}"));
    }
    parts.join(", ")
}

/// A circle read from a saved `<matrix A0..A5>`: centre, squared radius, and
/// the scale of the terms the squared radius is the difference of.
///
/// GeoGebra writes `A0 x^2 + A1 y^2 + A2 + 2 A3 x y + 2 A4 x + 2 A5 y = 0`.
struct SavedCircle {
    cx: f64,
    cy: f64,
    r2: f64,
    /// `cx^2 + cy^2 + |A2 / A0|`: `r2` is a difference of terms of this size,
    /// so its rounding error is a few units of `1e-16` times it.
    scale: f64,
}

/// How many units of `f64::EPSILON * scale` the squared radius recovered from
/// a saved matrix may be off. The matrix elements are computed by GeoGebra
/// from the centre and the radius with a handful of multiplications and
/// additions (each at most one rounding error of a term of size `scale`), and
/// the recovery subtracts terms of the same size: about ten roundings, with a
/// safety factor of four.
const MATRIX_ULPS: f64 = 40.0;

fn saved_circle(el: &Element) -> Option<Option<SavedCircle>> {
    let m = el.matrix.as_ref()?;
    let a: Option<Vec<f64>> = m.iter().map(|t| num(t)).collect();
    let Some(a) = a else {
        return Some(None);
    };
    if a[0] == 0.0 || !close(a[0], a[1]) || a[3].abs() > TOLERANCE * a[0].abs() {
        return Some(None);
    }
    let (cx, cy) = (-a[4] / a[0], -a[5] / a[0]);
    let r2 = cx * cx + cy * cy - a[2] / a[0];
    let scale = cx * cx + cy * cy + (a[2] / a[0]).abs();
    // A radius below the rounding noise of the matrix is a single point;
    // a clearly negative squared radius is no circle.
    (r2 >= -MATRIX_ULPS * f64::EPSILON * scale)
        .then_some(Some(SavedCircle { cx, cy, r2, scale }))
        .or(Some(None))
}

/// Whether the saved matrix, classified on a fresh record the way GeoGebra
/// classifies a conic (`ConicData::set_matrix`), is a single point. GeoGebra
/// reports a circle whose squared radius is within the tolerance of that
/// classification as a single point of radius 0, while the matrix it saves
/// still encodes the tiny radius (a circle of radius 4e-5 dilated by 2, for
/// example).
fn saved_single_point(el: &Element) -> bool {
    let Some(m) = el.matrix.as_ref() else {
        return false;
    };
    let a: Option<Vec<f64>> = m.iter().map(|t| num(t)).collect();
    let Some(Ok(a)) = a.map(<[f64; 6]>::try_from) else {
        return false;
    };
    let mut conic = ConicData::new();
    conic.set_matrix(a);
    conic.defined && conic.ty == ConicType::SinglePoint
}

/// Whether `def` makes the image of an object under a transformation.
fn is_transform_image(def: &Def) -> bool {
    match def {
        Def::Transform(_) => true,
        Def::GeneralConic(g) => matches!(
            g,
            GeneralConicDef::Translate { .. }
                | GeneralConicDef::Rotate { .. }
                | GeneralConicDef::Dilate { .. }
                | GeneralConicDef::MirrorPoint { .. }
                | GeneralConicDef::MirrorLine { .. }
        ),
        _ => false,
    }
}

/// Whether the saved element is defined, judged from its own data: `None`
/// when the file saved nothing to judge by.
fn saved_defined(el: &Element) -> Option<bool> {
    match el.kind.as_str() {
        "point" => el
            .coords
            .as_ref()
            .map(|_| coords3(el).is_some_and(|[_, _, z]| z != 0.0)),
        "vector" => el.coords.as_ref().map(|_| coords3(el).is_some()),
        // A degenerate segment (equal end points) is a defined object whose
        // saved line is (0, 0, 0).
        "segment" => el.coords.as_ref().map(|_| coords3(el).is_some()),
        "line" | "ray" => el
            .coords
            .as_ref()
            .map(|_| coords3(el).is_some_and(|[a, b, _]| a != 0.0 || b != 0.0)),
        // A saved matrix of finite numbers is a defined conic, whatever
        // conic it is (`saved_circle` reads only the circles among them);
        // a matrix with no numbers in it is not.
        "conic" => el.matrix.as_ref().map(|m| {
            m.iter()
                .map(|t| num(t))
                .collect::<Option<Vec<f64>>>()
                .is_some_and(|v| v.iter().all(|x| x.is_finite()) && v.iter().any(|x| *x != 0.0))
        }),
        // an infinite number is a defined value, an infinite angle is not
        "numeric" => el
            .value
            .as_ref()
            .map(|v| num(v).is_some() || saved_infinity(v).is_some()),
        "angle" => el.value.as_ref().map(|v| num(v).is_some()),
        "boolean" => el
            .value
            .as_deref()
            .map(|v| matches!(v.trim(), "true" | "false")),
        _ => None,
    }
}

/// Compares what the kernel computed with what the file saved for the same
/// object. `Some((saved, computed))` for a disagreement: a different value,
/// or one side defined and the other not. `None` when they agree or when the
/// file saved nothing to compare with. Explicit polygons also provide the
/// ordered vertex labels in the command and their coordinates in the elements.
fn cross_check(
    el: &Element,
    c: &Construction,
    id: Id,
    expression: Option<&str>,
    polygon: Option<&[[f64; 2]]>,
) -> Option<(String, String)> {
    let kind = c.kind(id);
    // Saved rays carry their line coefficients even without a finite start.
    let value = if kind == Kind::Ray {
        c.stored_value(id)
    } else {
        c.value(id)
    };
    let kind_matches = kind_agrees(&el.kind, kind);
    // GeoGebra writes independent text as a quoted expression. Dependent
    // texts normally save their definition or command and omit the result.
    let saved_string = (el.kind == "text")
        .then(|| {
            el.value.as_deref().or_else(|| {
                let body = expression?.trim().strip_prefix('"')?.strip_suffix('"')?;
                (!body.contains('"')).then_some(body)
            })
        })
        .flatten();
    let report = || {
        let saved =
            saved_string.map_or_else(|| saved_text(el), |s| format!("value {:?}", quote(s)));
        let computed = if let Some(text) = c.text(id) {
            format!("{kind}, {:?}", quote(text))
        } else if kind == Kind::Conic {
            format!(
                "{kind}, {value:?}, matrix {:?}",
                c.conic(id).map(|d| d.matrix)
            )
        } else {
            format!("{kind}, {value:?}")
        };
        let saved = if !saved.is_empty() {
            if kind_matches {
                saved
            } else {
                format!("{} {saved}", el.kind)
            }
        } else if let Some(exp) = expression {
            format!("{} definition `{}`", el.kind, quote(exp))
        } else if let Some(points) = polygon {
            format!("{} vertices {}", el.kind, points.len())
        } else {
            el.kind.clone()
        };
        Some((saved, computed))
    };
    if !kind_matches {
        return report();
    }
    if polygon.is_some_and(|points| !polygon_agrees(c, id, points)) {
        return report();
    }
    if let Some(saved) = saved_string {
        return (c.text(id) != Some(saved)).then(report).flatten();
    }
    if let Some(exp) = expression {
        let exp = exp.trim();
        match kind {
            Kind::Function => {
                let body = exp.split_once('=').map_or(exp, |(_, body)| body.trim());
                if let Some(expected) = num(body) {
                    if ![-1.0, 0.0, 1.0].into_iter().all(|x| {
                        c.function_value(id, x)
                            .is_some_and(|actual| close(actual, expected))
                    }) {
                        return report();
                    }
                }
            }
            Kind::List => {
                if let Some(parts) = geo_kernel::expr::list_literal_elements(exp) {
                    if let Some(list) = c.elements(id) {
                        if !list_agrees(list, parts) {
                            return report();
                        }
                    } else if parts.iter().all(|s| num(s).is_some()) {
                        return report();
                    }
                }
            }
            _ => {}
        }
    }
    if !value.is_defined() && kind == Kind::Point {
        if let Some([sx, sy, sz]) = coords3(el) {
            match c.stored_value(id) {
                Value::NormalizedPointAtInfinity { x, y } => {
                    return (!(sz == 1.0 && close(x, sx) && close(y, sy)))
                        .then(report)
                        .flatten();
                }
                // Cartesian overflow leaves the XML triple available to
                // geometry. Check that the importer kept all three fields.
                Value::HomogeneousPoint { x, y, z }
                    if sz != 0.0 && (!(sx / sz).is_finite() || !(sy / sz).is_finite()) =>
                {
                    let saved = [sx, sy, sz];
                    let computed = [x, y, z];
                    return (computed != saved && computed != saved.map(|n| -n))
                        .then(report)
                        .flatten();
                }
                _ => {}
            }
        }
    }
    let saved_defined =
        saved_defined(el).or_else(|| expression.filter(|exp| exp.trim() == "?").map(|_| false));
    match (value.is_defined(), saved_defined) {
        (false, Some(true)) | (true, Some(false)) => return report(),
        (false, _) | (_, None) => return None,
        (true, Some(true)) => {}
    }
    let agrees: Option<bool> = match value {
        Value::Point { x, y } => {
            coords3(el).map(|[sx, sy, sz]| close(x, sx / sz) && close(y, sy / sz))
        }
        Value::Vector { x, y } => coords3(el).map(|[sx, sy, _]| close(x, sx) && close(y, sy)),
        Value::Line { a, b, c } | Value::Ray { a, b, c, .. } => {
            coords3(el).map(|saved| same_line([a, b, c], saved))
        }
        Value::Segment { x1, y1, x2, y2 } => coords3(el).map(|saved| {
            if x1 == x2 && y1 == y2 {
                // GeoGebra saves the line of a degenerate segment as (0, 0, 0).
                saved == [0.0, 0.0, 0.0]
            } else {
                same_line([y1 - y2, x2 - x1, x1 * y2 - y1 * x2], saved)
            }
        }),
        Value::Circle { cx, cy, r } => saved_circle(el).map(|s| {
            s.is_some_and(|s| {
                // Centre and radius at the golden rules. The saved radius comes
                // from a difference of terms of size `scale`, so it is no more
                // precise than that: its error is bounded by the rounding of the
                // matrix, not by 1e-9 of the (possibly tiny) radius.
                let rs = s.r2.max(0.0).sqrt();
                let r2_error = MATRIX_ULPS * f64::EPSILON * s.scale;
                let radius_ok = close(r, rs)
                    || (r * r - s.r2).abs() <= r2_error
                    || (r == 0.0 && is_transform_image(&c.def(id)) && saved_single_point(el));
                close(cx, s.cx) && close(cy, s.cy) && radius_ok
            })
        }),
        Value::Conic { .. } => matrix6(el)
            .zip(c.conic(id))
            .map(|(saved, computed)| same_matrix(saved, computed.matrix)),
        Value::Number(n) if n.is_infinite() => el
            .value
            .as_deref()
            .map(|saved| saved_infinity(saved) == Some(n)),
        Value::Number(n) => el.value.as_deref().and_then(num).map(|s| {
            let s = if kind == Kind::Angle {
                let raw = if expression.is_none() && c.def(id).is_free() {
                    free_angle_raw(el).unwrap_or(s)
                } else {
                    s
                };
                c.angle_style(id).map_or(raw, |style| style.value(raw))
            } else {
                s
            };
            close(n, s)
        }),
        Value::Bool(b) => el
            .value
            .as_deref()
            .map(|v| v.trim() == if b { "true" } else { "false" }),
        _ => None,
    };
    (agrees == Some(false)).then(report).flatten()
}

fn polygon_agrees(c: &Construction, id: Id, saved: &[[f64; 2]]) -> bool {
    let mut vertices = c.vertices(id);
    saved.iter().all(|&[x, y]| {
        vertices
            .next()
            .flatten()
            .is_some_and(|(px, py)| close(x, px) && close(y, py))
    }) && vertices.next().is_none()
}

fn kind_agrees(saved: &str, actual: Kind) -> bool {
    let expected = match saved {
        "point" => Kind::Point,
        "vector" => Kind::Vector,
        "line" => Kind::Line,
        "ray" => Kind::Ray,
        "segment" => Kind::Segment,
        "conic" => return matches!(actual, Kind::Conic | Kind::Circle),
        "conicpart" => return matches!(actual, Kind::Arc | Kind::Sector),
        "numeric" => Kind::Number,
        "button" => Kind::Button,
        "textfield" => Kind::InputBox,
        "image" => Kind::Image,
        "piechart" => Kind::PieChart,
        "angle" => Kind::Angle,
        "boolean" => Kind::Boolean,
        "function" => Kind::Function,
        "functionnvar" => Kind::FunctionNVar,
        "list" => Kind::List,
        "polygon" => Kind::Polygon,
        "polyline" => Kind::PolyLine,
        "text" => Kind::Text,
        "locus" => Kind::Locus,
        "penstroke" => Kind::PenStroke,
        "curvecartesian" => Kind::Curve,
        "implicitpoly" => Kind::ImplicitCurve,
        _ => return true,
    };
    expected == actual
}

fn matrix6(el: &Element) -> Option<[f64; 6]> {
    let mut matrix = [0.0; 6];
    for (slot, text) in matrix.iter_mut().zip(el.matrix.as_ref()?) {
        *slot = num(text)?;
    }
    Some(matrix)
}

fn same_matrix(saved: [f64; 6], actual: [f64; 6]) -> bool {
    let pivot = (0..6)
        .max_by(|&a, &b| saved[a].abs().total_cmp(&saved[b].abs()))
        .unwrap();
    saved[pivot] != 0.0
        && actual[pivot] != 0.0
        && actual.iter().all(|x| x.is_finite())
        && (0..6).all(|i| close(saved[i] / saved[pivot], actual[i] / actual[pivot]))
}

fn list_agrees(list: geo_kernel::expr::List<'_>, parts: Vec<&str>) -> bool {
    list.len() == parts.len()
        && list
            .iter()
            .zip(parts)
            .all(|(actual, saved)| list_item_agrees(actual, saved))
}

fn list_item_agrees(actual: geo_kernel::expr::Element<'_>, saved: &str) -> bool {
    use geo_kernel::expr::Element as ListElement;
    if let Some(n) = num(saved) {
        matches!(actual, ListElement::Value(Kind::Number, Value::Number(v)) if close(v, n))
    } else if matches!(saved, "true" | "false") {
        matches!(actual, ListElement::Value(Kind::Boolean, Value::Bool(v)) if v == (saved == "true"))
    } else if let Some(parts) = geo_kernel::expr::list_literal_elements(saved) {
        matches!(actual, ListElement::List(inner) if list_agrees(inner, parts))
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::import;
    use geo_kernel::{script, Value};

    #[test]
    fn saved_text_verification_reports_exact_string_differences() {
        use super::cross_check;
        use crate::xml::{parse, Item};
        let xml = crate::archive::extract_xml(include_bytes!(
            "../../../golden/ggb/saved/saved-texts-001.ggb"
        ))
        .unwrap();
        let mut doc = parse(&xml).unwrap();
        let label = "note";
        let (el, exp) = (
            doc.items
                .iter()
                .find_map(|item| match item {
                    Item::Element(el) if el.label == label => Some(el.as_ref()),
                    _ => None,
                })
                .unwrap(),
            doc.items
                .iter()
                .find_map(|item| match item {
                    Item::Expression { label: name, exp } if name == label => Some(exp.as_str()),
                    _ => None,
                })
                .unwrap(),
        );
        let c = script::load(r#"note = "Survey note second line""#).unwrap();
        assert!(cross_check(el, &c, c.id(label).unwrap(), Some(exp), None).is_some());

        // Supply a cached value alongside the saved expression to exercise
        // mismatch reporting through the converter's normal entry point.
        for item in &mut doc.items {
            if let Item::Element(el) = item {
                if el.label == label {
                    el.value = c.text(c.id(label).unwrap()).map(str::to_owned);
                }
            }
        }
        let imported = super::convert(doc).unwrap();
        assert_eq!(imported.mismatched.len(), 1, "{:?}", imported.mismatched);
        let mismatch = &imported.mismatched[0];
        assert_eq!(mismatch.label, label);
        assert!(mismatch.saved.contains("Survey note second line"));
        assert!(mismatch.computed.contains(r"Survey note\nsecond line"));
    }

    #[test]
    fn saved_verification_detects_different_kinds_and_serialized_values() {
        use super::{coords3, cross_check};
        use crate::xml::{parse, Item};
        let doc = parse(include_str!(
            "../../../golden/ggb/saved/saved-review-001.xml"
        ))
        .unwrap();
        let element = |label| {
            doc.items
                .iter()
                .find_map(|item| match item {
                    Item::Element(el) if el.label == label => Some(el.as_ref()),
                    _ => None,
                })
                .unwrap()
        };
        let exp = |label| {
            doc.items.iter().find_map(|item| match item {
                Item::Expression { label: l, exp } if l == label => Some(exp.as_str()),
                _ => None,
            })
        };
        let c = script::load(
            "chosen = (1.4142135623730951, 1)\nflat(x) = 6\nq = {(1, 1), 1/3, 6, 2pi}\n\
             A = (1, 1)\nB = (3, 4)\nC = (2, 5)\ntri = Polygon(A, B, C)\n\
             n = 6\nel = Ellipse(A, B, n)",
        )
        .unwrap();
        for label in ["chosen", "flat", "q", "el"] {
            assert!(
                cross_check(element(label), &c, c.id(label).unwrap(), exp(label), None).is_some(),
                "{label}"
            );
        }
        let points: Vec<_> = ["A", "B", "C"]
            .into_iter()
            .map(|label| {
                let [x, y, z] = coords3(element(label)).unwrap();
                [x / z, y / z]
            })
            .collect();
        assert!(cross_check(
            element("tri"),
            &c,
            c.id("tri").unwrap(),
            None,
            Some(&points)
        )
        .is_some());
    }

    #[test]
    fn saved_transform_ray_uses_hidden_helpers() {
        let imported = import(include_bytes!(
            "../../../golden/ggb/transform/transform-dilate-007.xml"
        ))
        .unwrap();
        assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
        let c = script::load(&imported.script.join("\n")).unwrap();
        let helpers: Vec<_> = c
            .ids()
            .filter(|&id| c.label(id).starts_with('\0'))
            .collect();
        assert_eq!(helpers.len(), 2);
        assert!(c.id("dir").is_none() && c.id("img").is_none());
    }

    #[test]
    fn skipped_arc_leaves_no_helpers() {
        let xml = include_str!("../../../golden/ggb/transform/transform-arcpart-005.xml");
        // Use a line for the end point so the arc fails after expanding its
        // transformed conic into a helper.
        let broken = xml.replace(
            "a0=\"Shear[k, n, f]\" a1=\"P&apos;\" a2=\"Q&apos;\"",
            "a0=\"Shear[k, n, f]\" a1=\"P&apos;\" a2=\"n\"",
        );
        assert_ne!(xml, broken);
        let imported = import(broken.as_bytes()).unwrap();
        assert_eq!(imported.skipped.len(), 1, "{:?}", imported.skipped);
        assert_eq!(imported.skipped[0].label, "m");
        let c = script::load(&imported.script.join("\n")).unwrap();
        assert!(c
            .ids()
            .all(|id| !c.label(id).starts_with('\0') && c.label(id) != "img"));
    }

    #[test]
    fn saved_ray_uses_the_position_vector_of_a_point() {
        let imported = import(include_bytes!(
            "../../../golden/transformsfix/import-ray.xml"
        ))
        .unwrap();
        assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
        let mut c = script::load(&imported.script.join("\n")).unwrap();
        let ray = c.id("r").unwrap();
        assert_eq!(
            c.value(ray),
            Value::Ray {
                x: 1.0,
                y: 2.0,
                a: -4.0,
                b: 3.0,
                c: -2.0
            }
        );
        c.move_point(c.id("B").unwrap(), -1.0, 3.0).unwrap();
        assert_eq!(
            c.value(ray),
            Value::Ray {
                x: 1.0,
                y: 2.0,
                a: -3.0,
                b: -1.0,
                c: 5.0
            }
        );
    }
}
