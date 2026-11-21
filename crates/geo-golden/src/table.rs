//! A reader for the small CSV tables of the repository
//! (`tools/command-matrix/forms.csv`, `docs/command-matrix.csv`,
//! `docs/kinds.csv`): comma-separated fields, a field in double quotes may
//! hold commas, and `""` inside quotes is one quote. Every record is one line.

use std::collections::BTreeMap;

/// A table with a header row.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Parses `text`; every row must have as many fields as the header.
    pub fn parse(text: &str, source: &str) -> Result<Table, String> {
        let mut lines = text.lines().enumerate();
        let header = match lines.next() {
            Some((_, l)) => split_record(l).map_err(|e| format!("{source}:1: {e}"))?,
            None => return Err(format!("{source}: empty file")),
        };
        let mut rows = Vec::new();
        for (i, line) in lines {
            let row = split_record(line).map_err(|e| format!("{source}:{}: {e}", i + 1))?;
            if row.len() != header.len() {
                return Err(format!(
                    "{source}:{}: {} fields, the header has {}",
                    i + 1,
                    row.len(),
                    header.len()
                ));
            }
            rows.push(row);
        }
        Ok(Table { header, rows })
    }

    /// The index of the column `name`.
    pub fn column(&self, name: &str) -> Result<usize, String> {
        self.header
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| format!("no column `{name}`"))
    }

    /// Every row as a map from column name to field.
    pub fn records(&self) -> Vec<BTreeMap<&str, &str>> {
        self.rows
            .iter()
            .map(|r| {
                self.header
                    .iter()
                    .map(String::as_str)
                    .zip(r.iter().map(String::as_str))
                    .collect()
            })
            .collect()
    }
}

/// Splits one CSV record into its fields.
pub fn split_record(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    let mut was_quoted = false;
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(ch);
            }
        } else if ch == ',' {
            fields.push(std::mem::take(&mut field));
            was_quoted = false;
        } else if ch == '"' && field.is_empty() && !was_quoted {
            quoted = true;
            was_quoted = true;
        } else if was_quoted {
            return Err(format!("text after a closing quote: `{line}`"));
        } else {
            field.push(ch);
        }
    }
    if quoted {
        return Err(format!("unterminated quote: `{line}`"));
    }
    fields.push(field);
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_hold_commas_and_quotes() {
        assert_eq!(
            split_record(r#"a,"b, c","say ""hi""",,d"#).unwrap(),
            ["a", "b, c", "say \"hi\"", "", "d"]
        );
        assert_eq!(split_record("").unwrap(), [""]);
        assert!(split_record(r#"a,"b"#).is_err());
        assert!(split_record(r#""a"b,c"#).is_err());
    }

    #[test]
    fn rows_need_the_header_width() {
        let t = Table::parse("x,y\n1,2\n\"3,4\",5\n", "t").unwrap();
        assert_eq!(t.rows, [["1", "2"], ["3,4", "5"]]);
        assert_eq!(t.column("y"), Ok(1));
        assert_eq!(t.records()[1]["x"], "3,4");
        assert!(Table::parse("x,y\n1\n", "t").is_err());
    }
}
