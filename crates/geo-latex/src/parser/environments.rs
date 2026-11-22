use super::commands::{dimension, number};
use super::{Align, Grid, Length, Node, Parser, Result, Stop};
use crate::Style;

impl<'a> Parser<'a> {
    pub(super) fn unsupported_environment(
        &mut self,
        environment: &str,
        start: &'a str,
    ) -> Result<Node<'a>> {
        let mut depth = 1;
        while let Ok(ch) = self.next() {
            if ch != '\\' {
                continue;
            }
            let name = self.control()?;
            if matches!(name, "begin" | "end") && self.raw_group()? == environment {
                if name == "begin" {
                    depth += 1;
                } else {
                    depth -= 1;
                }
                if depth == 0 {
                    return Ok(Node::Fallback(&start[..start.len() - self.rest.len()]));
                }
            }
        }
        Err(())
    }

    pub(super) fn environment(&mut self, env: &str) -> Result<Option<Node<'a>>> {
        let delimiters = match env {
            "pmatrix" => ('(', ')'),
            "bmatrix" => ('[', ']'),
            "Bmatrix" => ('{', '}'),
            "vmatrix" => ('|', '|'),
            "Vmatrix" => ('‖', '‖'),
            "cases" => ('{', '.'),
            "rcases" => ('.', '}'),
            "matrix" | "smallmatrix" | "array" | "tabular" | "align" | "align*" | "aligned"
            | "alignedat" | "alignat" | "alignat*" | "gather" | "gather*" | "gathered"
            | "split" | "eqnarray" | "eqnarray*" => ('.', '.'),
            _ => return Ok(None),
        };
        let mut grid = Grid {
            rows: Vec::new(),
            columns: Vec::new(),
            vertical: Vec::new(),
            horizontal: Vec::new(),
            gaps: Vec::new(),
            aligned: env.starts_with("align") || matches!(env, "split" | "eqnarray" | "eqnarray*"),
            text: env == "tabular",
        };
        if matches!(env, "array" | "tabular") {
            let spec = self.raw_group()?;
            let mut columns = Parser {
                rest: spec,
                depth: 0,
                text: false,
                colors: Default::default(),
            };
            while {
                columns.space();
                !columns.rest.is_empty()
            } {
                match columns.next()? {
                    '|' => grid.vertical.push(grid.columns.len()),
                    ch @ ('l' | 'c' | 'r' | 'L' | 'C' | 'R') => {
                        let align = match ch.to_ascii_lowercase() {
                            'l' => Align::Left,
                            'r' => Align::Right,
                            _ => Align::Center,
                        };
                        let width = if ch.is_ascii_uppercase() {
                            table_length(columns.raw_group()?).ok_or(())?
                        } else {
                            Length::Em(0.0)
                        };
                        grid.columns.push((align, width));
                    }
                    _ => return Err(()),
                }
            }
        } else if env.ends_with("cases") {
            grid.columns.extend([(Align::Left, Length::Em(0.0)); 2]);
        } else if env.contains("alignat") {
            self.raw_group()?;
        }
        let old = self.text;
        self.text |= grid.text;
        self.space();
        let mut unclosed = false;
        loop {
            while self.eat_control("hline") {
                grid.horizontal.push(grid.rows.len());
                self.space();
            }
            if self.eat_control("end") {
                if self.raw_group()? != env {
                    return Err(());
                }
                break;
            }
            let mut cells = vec![self.cell()?];
            while self.eat("&") {
                if cells.len() == 128 {
                    return Err(());
                }
                cells.push(self.cell()?);
            }
            if grid.rows.len() == 512 {
                return Err(());
            }
            grid.rows.push(cells);
            if self.eat(r"\\") || self.eat_control("cr") {
                self.eat("*");
                let gap = self
                    .raw_option()?
                    .and_then(table_length)
                    .unwrap_or(Length::Em(0.0));
                grid.gaps.push(gap);
            } else if self.rest.is_empty() {
                // A missing \end: the grid is shown as built, flagged as
                // partly broken (TeX inserts the \end it asks for).
                unclosed = true;
                break;
            } else if !self.at_control("end") {
                return Err(());
            }
            self.space();
        }
        self.text = old;
        let grid = Node::Grid(grid);
        let body = if env == "smallmatrix" {
            Node::Styled(Style::Script, Some(Box::new(grid)))
        } else {
            grid
        };
        let node = if delimiters == ('.', '.') {
            body
        } else {
            Node::Delimited(delimiters.0, Box::new(body), delimiters.1)
        };
        Ok(Some(if unclosed {
            Node::Row(vec![node, Node::Broken])
        } else {
            node
        }))
    }

    fn cell(&mut self) -> Result<Node<'a>> {
        self.space();
        if self.eat_control("cellcolor") {
            let raw = self.raw_group()?;
            let content = Box::new(self.row(Stop::Grid));
            return self
                .resolve_color(None, raw)
                .map(|c| Node::CellColor(c, content))
                .ok_or(());
        }
        Ok(self.row(Stop::Grid))
    }
}

// TableText emits bare point widths and row skips. Explicit TeX
// units are also understood in handmade array specifications.
fn table_length(source: &str) -> Option<Length> {
    number(source)
        .map(|n| Length::Points(n.clamp(-2048.0, 2048.0)))
        .or_else(|| dimension(source))
}
