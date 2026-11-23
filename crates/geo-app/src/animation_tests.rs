//! The play button of the drawing: it shows while a slider animates, pauses
//! the animation and plays it again.

use crate::headless::{press, Headless, Profile};
use crate::workbench::Workbench;
use eframe::egui::{Event, PointerButton};
use geo_app_core::text::{self, Label};

const DT: f64 = 1.0 / 30.0;

fn slider_value(h: &Headless) -> f64 {
    let c = h.app.wb.construction();
    match c.value(c.id("a").unwrap()) {
        geo_kernel::Value::Number(v) => v,
        v => panic!("{v:?}"),
    }
}

fn click_button(h: &mut Headless, label: &str) {
    let button = h
        .widgets()
        .into_iter()
        .find(|w| w.label == label)
        .unwrap_or_else(|| panic!("no `{label}` button"));
    let at = button.rect.center();
    h.frame(
        DT,
        vec![
            Event::PointerMoved(at),
            press(at, PointerButton::Primary, true),
        ],
    );
    h.frame(DT, vec![press(at, PointerButton::Primary, false)]);
}

#[test]
fn the_play_button_pauses_and_plays_the_animation() {
    let wb = Workbench::from_script("f(x) = x\ng = SlowPlot(f)").unwrap();
    let mut h = Headless::new(wb, None, Profile::Desktop, (1200.0, 800.0), 1.0);
    for _ in 0..10 {
        h.frame(DT, vec![]);
    }
    assert!(h.app.wb.is_animating());
    assert!(slider_value(&h) > 0.0);
    click_button(&mut h, text::label(Label::PauseAnimation));
    assert!(!h.app.wb.is_animating());
    let paused = slider_value(&h);
    for _ in 0..10 {
        h.frame(DT, vec![]);
    }
    assert_eq!(slider_value(&h), paused);
    click_button(&mut h, text::label(Label::PlayAnimation));
    assert!(h.app.wb.is_animating());
    for _ in 0..10 {
        h.frame(DT, vec![]);
    }
    assert!(slider_value(&h) > paused);
}

#[test]
fn no_play_button_shows_without_an_animation() {
    let wb = Workbench::from_script("f(x) = x").unwrap();
    let mut h = Headless::new(wb, None, Profile::Desktop, (1200.0, 800.0), 1.0);
    h.frame(DT, vec![]);
    let names = [Label::PlayAnimation, Label::PauseAnimation].map(text::label);
    assert!(!h
        .widgets()
        .iter()
        .any(|w| names.contains(&w.label.as_str())));
}
