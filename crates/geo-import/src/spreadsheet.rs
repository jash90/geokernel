//! Spreadsheet settings in GeoGebra's XML and editor inputs in the archive.

use std::collections::BTreeMap;

use geo_kernel::spreadsheet::Address;

pub(crate) const METADATA_FILE: &str = "geokernel_spreadsheet.xml";
pub(crate) const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpreadsheetView {
    pub visible: Option<bool>,
    pub(crate) settings: Vec<Setting>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Setting {
    pub name: &'static str,
    pub attributes: Vec<(&'static str, String)>,
}

impl SpreadsheetView {
    pub fn selected(&self) -> Option<Address> {
        self.settings.iter().find(|tag| tag.name == "selection")?;
        let row = self
            .attribute("selection", "row")
            .unwrap_or("0")
            .parse::<u16>()
            .ok()?;
        let column = self
            .attribute("selection", "column")
            .unwrap_or("0")
            .parse::<u16>()
            .ok()?;
        Address::new(column.checked_add(1)?, row.checked_add(1)?)
    }

    pub fn set_selected(&mut self, cell: Address) {
        let index = self.settings.iter().position(|tag| tag.name == "selection");
        let tag = match index {
            Some(index) => &mut self.settings[index],
            None => {
                self.settings.push(Setting {
                    name: "selection",
                    attributes: vec![("hScroll", "0".into()), ("vScroll", "0".into())],
                });
                self.settings.last_mut().expect("added selection")
            }
        };
        tag.attributes
            .retain(|(key, _)| !matches!(*key, "row" | "column"));
        tag.attributes.extend([
            ("row", cell.row.saturating_sub(1).to_string()),
            ("column", cell.column.saturating_sub(1).to_string()),
        ]);
    }

    pub fn preferred_cell_size(&self) -> [Option<u16>; 2] {
        ["width", "height"].map(|key| self.attribute("prefCellSize", key)?.parse().ok())
    }

    fn attribute(&self, tag: &str, key: &str) -> Option<&str> {
        self.settings
            .iter()
            .find(|setting| setting.name == tag)?
            .attributes
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.as_str())
    }
}

pub(crate) type Cells = BTreeMap<Address, String>;

pub(crate) fn setting(name: &str) -> Option<(&'static str, &'static [&'static str])> {
    Some(match name {
        "size" => ("size", &["width", "height"]),
        "prefCellSize" => ("prefCellSize", &["width", "height"]),
        "dimensions" => ("dimensions", &["rows", "columns"]),
        "spreadsheetColumn" => ("spreadsheetColumn", &["id", "width"]),
        "spreadsheetRow" => ("spreadsheetRow", &["id", "height"]),
        "selection" => ("selection", &["hScroll", "vScroll", "column", "row"]),
        "spreadsheetCellFormat" => ("spreadsheetCellFormat", &["formatMap"]),
        "layout" => (
            "layout",
            &[
                "showFormulaBar",
                "showGrid",
                "showColumnHeader",
                "showRowHeader",
                "showHScrollBar",
                "showVScrollBar",
                "allowSpecialEditor",
                "allowToolTips",
                "equalsRequired",
                "autoComplete",
            ],
        ),
        _ => return None,
    })
}
