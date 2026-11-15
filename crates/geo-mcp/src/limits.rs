//! Every bound the server puts on its input, in one place (listed in the
//! README). A request beyond one is refused with an error; nothing grows
//! with the number of calls or frames.

/// The longest request line, in bytes. A longer line is answered with an
/// error and skipped up to the next newline.
pub const MAX_LINE: usize = 16 << 20;
/// The largest document `reset` and `open` take (a script inline, a file
/// by path or as base64), in bytes.
pub const MAX_DOCUMENT: u64 = 8 << 20;
/// The most frames one call runs: the frames of `input`, the moves of
/// `drag`, the clicks of `click`.
pub const MAX_FRAMES: usize = 10_000;
/// The most events one frame of `input` takes.
pub const MAX_EVENTS: usize = 1_000;
/// The longest text of a `text` event or of `type_line`, in bytes.
pub const MAX_TEXT: usize = 64 << 10;
/// Coordinates beyond this many points (or world units) are refused.
pub const MAX_COORD: f64 = 1.0e6;
/// The most empty frames a settle runs after an action.
pub const SETTLE_MAX: usize = 120;
/// The longest delay between two frames, in seconds.
pub const MAX_DT: f64 = 3600.0;
