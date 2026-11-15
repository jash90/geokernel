//! The pointer events a platform layer makes of touches, as egui-winit 0.37
//! does in `State::on_touch`.

use super::press;
use eframe::egui::{self, Event, PointerButton, Pos2, TouchPhase};

/// What egui-winit 0.37 makes of touches (`State::on_touch`): every touch
/// event, and pointer events for the finger it follows as the mouse, which
/// is the one that went down while no finger was followed. After that finger
/// lifts no finger is followed, so the moves of a finger that stays are
/// reported as the mouse's, and its end as a release (at the last pointer
/// position, if one is known) that no press came before.
#[derive(Clone, Debug, Default)]
pub struct TouchEmulation {
    pointer: Option<u64>,
    pos: Option<Pos2>,
}

impl TouchEmulation {
    /// The events of one touch event of finger `id`.
    pub fn touch(&mut self, id: u64, phase: TouchPhase, pos: Pos2) -> Vec<Event> {
        let mut events = vec![Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(id),
            phase,
            pos,
            force: None,
        }];
        if self.pointer.is_some_and(|p| p != id) {
            return events;
        }
        match phase {
            TouchPhase::Start => {
                self.pointer = Some(id);
                self.pos = Some(pos);
                events.extend([
                    Event::PointerMoved(pos),
                    press(pos, PointerButton::Primary, true),
                ]);
            }
            TouchPhase::Move => {
                self.pos = Some(pos);
                events.push(Event::PointerMoved(pos));
            }
            TouchPhase::End => {
                self.pointer = None;
                events.extend(
                    self.pos
                        .take()
                        .map(|p| press(p, PointerButton::Primary, false)),
                );
                events.push(Event::PointerGone);
            }
            TouchPhase::Cancel => {
                self.pointer = None;
                self.pos = None;
                events.push(Event::PointerGone);
            }
        }
        events
    }
}
