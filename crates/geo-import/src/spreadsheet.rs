//! Spreadsheet settings in GeoGebra's XML and editor inputs in the archive.

use std::collections::BTreeMap;
use std::sync::Arc;

use geo_kernel::spreadsheet::{Address, MAX_COLUMNS, MAX_ROWS};

pub(crate) const METADATA_FILE: &str = "geokernel_spreadsheet.xml";
pub(crate) const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpreadsheetView {
    pub visible: Option<bool>,
    pub(crate) settings: Vec<Setting>,
    pub(crate) grid: SpreadsheetGrid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Setting {
    pub name: &'static str,
    pub attributes: Vec<(&'static str, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpreadsheetGrid {
    pub show_lines: bool,
    pub columns: CellAxis,
    pub rows: CellAxis,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellAxis {
    cell_size: u16,
    count: u16,
    edges: Option<Arc<[u32]>>,
}

impl SpreadsheetGrid {
    pub const DEFAULT: Self = Self {
        show_lines: true,
        columns: CellAxis {
            cell_size: 74,
            count: MAX_COLUMNS,
            edges: None,
        },
        rows: CellAxis {
            cell_size: 24,
            count: MAX_ROWS,
            edges: None,
        },
    };
}

impl Default for SpreadsheetGrid {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl CellAxis {
    /// Position of the edge after `index` cells, measured from the first cell.
    pub fn edge(&self, index: u16) -> f32 {
        let index = index.min(self.count);
        self.edges
            .as_deref()
            .map_or(u32::from(index) * u32::from(self.cell_size), |edges| {
                edges[usize::from(index)]
            }) as f32
    }

    /// One-based index of the cell at an offset from the first cell.
    pub fn cell_at(&self, position: f32) -> u16 {
        let position = position.max(0.0) as u32;
        match &self.edges {
            Some(edges) => (edges.partition_point(|&edge| edge <= position) as u16).min(self.count),
            None => (position / u32::from(self.cell_size) + 1).min(u32::from(self.count)) as u16,
        }
    }

    fn new(
        default: Self,
        preferred: Option<u16>,
        settings: &[Setting],
        tag: &str,
        dimension: &str,
        minimum: u16,
    ) -> Self {
        // Each positive minimum bounds the number of widgets per frame,
        // including with extreme dimensions in a file.
        let cell_size = preferred.unwrap_or(default.cell_size).max(minimum);
        let mut sizes = None;
        for setting in settings.iter().filter(|setting| setting.name == tag) {
            let index = setting
                .attribute("id")
                .and_then(|id| id.parse::<u16>().ok());
            let size = setting
                .attribute(dimension)
                .and_then(|size| size.parse::<f64>().ok())
                .filter(|size| size.is_finite() && *size > 0.0);
            if let (Some(index), Some(size)) = (index.filter(|&i| i < default.count), size) {
                sizes.get_or_insert_with(|| vec![cell_size; usize::from(default.count)])
                    [usize::from(index)] = (size.round() as u16).max(minimum);
            }
        }
        let edges = sizes.map(|sizes| {
            let mut edge = 0;
            std::iter::once(0)
                .chain(sizes.into_iter().map(|size| {
                    edge += u32::from(size);
                    edge
                }))
                .collect()
        });
        Self {
            cell_size,
            edges,
            ..default
        }
    }
}

impl Setting {
    fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.as_str())
    }
}

impl SpreadsheetView {
    pub fn grid(&self) -> &SpreadsheetGrid {
        &self.grid
    }

    pub(crate) fn prepare_grid(&mut self) {
        self.grid.show_lines = self.attribute("layout", "showGrid") != Some("false");
        let [width, height] = self.preferred_cell_size();
        let default = SpreadsheetGrid::DEFAULT;
        self.grid.columns = CellAxis::new(
            default.columns,
            width,
            &self.settings,
            "spreadsheetColumn",
            "width",
            32,
        );
        self.grid.rows = CellAxis::new(
            default.rows,
            height,
            &self.settings,
            "spreadsheetRow",
            "height",
            16,
        );
    }

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
            .attribute(key)
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

#[cfg(test)]
mod tests {
    #[test]
    fn tiny_saved_dimensions_have_a_visible_cell_bound() {
        let doc = crate::xml::parse(
            r#"<geogebra><spreadsheetView>
            <prefCellSize width="1" height="1"/>
            <spreadsheetColumn id="0" width="2"/>
            <spreadsheetRow id="0" height="2"/>
            </spreadsheetView><construction/></geogebra>"#,
        )
        .unwrap();
        let sheet = doc.spreadsheet_view.unwrap();
        let grid = sheet.grid();
        assert_eq!(grid.columns.edge(2), 64.0);
        assert_eq!(grid.rows.edge(2), 32.0);
        assert_eq!(grid.columns.cell_at(1400.0), 44);
        assert_eq!(grid.rows.cell_at(900.0), 57);
    }
}
