//! Controls carried by an imported construction. Script text is inert data.

use crate::{Construction, Def, Id, Kind};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Control {
    pub caption: String,
    pub scripts: Vec<ControlScript>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlScript {
    pub language: String,
    pub event: String,
    pub text: String,
}

impl Construction {
    pub fn control(&self, id: Id) -> Option<&Control> {
        let index = self
            .controls
            .binary_search_by_key(&id, |(id, _)| *id)
            .ok()?;
        Some(&self.controls[index].1)
    }

    /// Imported object scripts, including those attached to controls.
    pub fn object_scripts(&self, label: &str) -> &[ControlScript] {
        if let Some(scripts) = self.object_scripts.get(label) {
            return scripts;
        }
        self.id(label)
            .and_then(|id| self.control(id))
            .map_or(&[], |control| &control.scripts)
    }

    /// Changes a free checkbox and updates its mathematical dependents.
    pub fn set_boolean(&mut self, id: Id, value: bool) -> Result<usize, crate::kernel::Error> {
        self.last_update_changed_automatic_root = false;
        self.last_update_changed_list_record = false;
        let def = self
            .defs
            .get_mut(id.index())
            .ok_or(crate::kernel::Error::UnknownObject)?;
        let Def::Boolean(old) = def else {
            return Err(crate::kernel::Error::NotFree(self.label(id).to_owned()));
        };
        if *old == value {
            return Ok(0);
        }
        *old = value;
        self.values[id.index()] = crate::Value::Bool(value);
        Ok(self.propagate(id))
    }
}

// Encode arbitrary captions and inert script text as single-line fields.
pub fn encode_text(text: &str) -> String {
    encode_bytes(text.as_bytes())
}

pub(crate) fn encode_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(1 + 2 * bytes.len());
    encoded.push('_');
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

pub fn decode_text(encoded: &str) -> Result<String, String> {
    String::from_utf8(decode_bytes(encoded)?).map_err(|_| "invalid control text".into())
}

pub(crate) fn decode_bytes(encoded: &str) -> Result<Vec<u8>, String> {
    let encoded = encoded.strip_prefix('_').ok_or("invalid control text")?;
    if !encoded.is_ascii() || !encoded.len().is_multiple_of(2) {
        return Err("invalid control text".into());
    }
    (0..encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "invalid control text".into())
}

/// Uses a named boolean directly or adds an anonymous boolean expression.
pub fn visibility_boolean(c: &mut Construction, owner: &str, source: &str) -> Result<Id, String> {
    if let Some(id) = c.id(source) {
        return (c.kind(id) == Kind::Boolean)
            .then_some(id)
            .ok_or_else(|| format!("`{source}` is not a boolean"));
    }
    let label = format!("\0condition:{owner}");
    if crate::expr::argument_object(c, source, &label, Kind::Boolean) {
        Ok(c.id(&label).expect("the boolean was just added"))
    } else {
        Err("the visibility expression cannot be evaluated as a boolean".into())
    }
}

pub(crate) fn define(
    c: &mut Construction,
    label: &str,
    form: &str,
    args: &[&str],
) -> Result<Id, String> {
    let (def, caption, scripts) = match (form, args) {
        ("$button", [caption, scripts @ ..]) => (Def::Button, caption, scripts),
        ("$checkBox", [value, caption, scripts @ ..]) => (
            Def::Boolean(value.parse().map_err(|_| "invalid checkbox value")?),
            caption,
            scripts,
        ),
        ("$inputBox", [linked, caption, scripts @ ..]) => {
            let linked = if linked.is_empty() {
                None
            } else {
                Some(crate::script::object(c, linked)?)
            };
            (Def::InputBox(linked), caption, scripts)
        }
        _ => return Err("invalid control definition".into()),
    };
    let caption = decode_text(caption)?;
    let scripts = decode_scripts(scripts)?;
    let id = c.add(label, def).map_err(|e| e.to_string())?;
    c.controls.push((id, Control { caption, scripts }));
    Ok(id)
}

pub(crate) fn decode_scripts(fields: &[&str]) -> Result<Vec<ControlScript>, String> {
    let (scripts, []) = fields.as_chunks::<3>() else {
        return Err("invalid object script fields".into());
    };
    scripts
        .iter()
        .map(|&[language, event, text]| {
            Ok(ControlScript {
                language: decode_text(language)?,
                event: decode_text(event)?,
                text: decode_text(text)?,
            })
        })
        .collect()
}

pub fn encode_scripts(scripts: &[ControlScript]) -> impl Iterator<Item = String> + '_ {
    scripts.iter().flat_map(|script| {
        [&script.language, &script.event, &script.text].map(|field| encode_text(field))
    })
}
