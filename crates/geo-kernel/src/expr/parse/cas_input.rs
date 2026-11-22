use super::{Bin, Node, Parser, Tok, Ty, NONE};
use crate::expr::cas::Op;

impl Parser<'_> {
    pub(in crate::expr) fn cas_command(&mut self, name: &str) -> Result<Option<u32>, String> {
        let Some(op) = Op::named(name) else {
            return Ok(None);
        };
        if matches!(op, Op::Solve { .. }) {
            return self.parse_solve(name).map(Some);
        }
        let outer_x = self.uses_var;
        let outer_y = self.uses_y;
        self.uses_var = false;
        self.uses_y = false;
        let raw = self.arguments()?;
        let (x, y) = (self.uses_var, self.uses_y);
        let multivariate = raw.iter().any(|&index| {
            matches!(self.node(index), Node::FunctionObject(id) if self.c.kind(id) == crate::Kind::FunctionNVar)
        });
        self.uses_var = outer_x;
        self.uses_y = outer_y;
        let node = match raw[..] {
            [a, b] if op == Op::CommonDenominator => {
                if !x && !y && self.is_number(a) && self.is_number(b) {
                    Node::Bin(Bin::CommonDenominator, a, b)
                } else {
                    self.uses_var = true;
                    self.uses_y |= y || multivariate;
                    Node::Cas(op, a, b)
                }
            }
            [a] if op != Op::CommonDenominator => {
                if op.is_function() {
                    if matches!(
                        op,
                        Op::Factor | Op::IFactor | Op::CompleteSquare | Op::Polynomial
                    ) && !x
                        && !y
                        && self.checked_type(a)? != Ty::Function
                        && self.locals.is_empty()
                    {
                        return Err(format!("`{name}` needs a function"));
                    }
                    self.uses_var |= x
                        || y
                        || matches!(op, Op::Expand | Op::Simplify)
                        || self.checked_type(a)? == Ty::Function;
                    self.uses_y |= y || multivariate;
                } else if matches!(op, Op::NextPrime | Op::PreviousPrime) {
                    self.uses_var |= x;
                    self.uses_y |= y;
                }
                Node::Cas(op, a, NONE)
            }
            _ => return Err(format!("invalid arguments to `{name}`")),
        };
        Ok(Some(self.push(node)))
    }

    fn parse_solve(&mut self, name: &str) -> Result<u32, String> {
        let points = name == "PlotSolve";
        let commas = self.argument_commas();
        let hint = match commas[..] {
            [] => Hint {
                names: vec!["x".into(), "y".into(), "z".into()],
                width: 0,
                axes: [0, 1, 2],
                list: false,
                guess: false,
            },
            // `CmdSolve.process`: the plotted solve takes one argument.
            [comma] if !points => self.solution_hint(comma + 1)?,
            _ => return Err(format!("invalid arguments to `{name}`")),
        };
        let (outer_x, outer_y) = (self.uses_var, self.uses_y);
        let old = std::mem::replace(&mut self.cas_variables, hint.names);
        let old_split = std::mem::replace(&mut self.split_names, true);
        self.expect(&Tok::LParen, "(")?;
        let is_list = self.eat(&Tok::LBrace);
        let mut equations = vec![self.cas_equation()?];
        if is_list {
            while self.eat(&Tok::Comma) {
                equations.push(self.cas_equation()?);
            }
            self.expect(&Tok::RBrace, "}")?;
        }
        self.cas_variables = old;
        self.split_names = old_split;
        let mut guess = NONE;
        if let [comma] = commas[..] {
            if self.pos != comma {
                return Err("expected a list of solving variables".into());
            }
            if hint.guess {
                // `variable = value`: the value is an ordinary number.
                self.pos = comma + 3;
                guess = self.argument(false)?;
                if !self.is_number(guess) {
                    return Err("expected a number as the initial value".into());
                }
            } else {
                // The suffix was parsed and validated when choosing the variable scope.
                self.pos = self.toks[comma + 1..]
                    .iter()
                    .position(|t| t.tok == Tok::RParen)
                    .map_or(self.toks.len(), |offset| comma + 1 + offset);
            }
        }
        self.expect(&Tok::RParen, ")")?;
        (self.uses_var, self.uses_y) = (outer_x, outer_y);
        if equations.len() > 8 {
            return Err("at most eight polynomial equations can be solved together".into());
        }
        let is_list = is_list || matches!(equations.as_slice(), [arg] if self.is_list(*arg));
        let equations = self.wrap_list(&equations);
        let op = Op::Solve {
            assignments: matches!(name, "Solve" | "NSolve" | "PlotSolve"),
            approximate: matches!(name, "NSolve" | "NSolutions"),
            width: hint.width,
            axes: hint.axes,
            system: is_list,
            rows: hint.list,
            points,
        };
        Ok(self.push(Node::Cas(op, equations, guess)))
    }

    fn cas_equation(&mut self) -> Result<u32, String> {
        let left = self.argument(false)?;
        if self.eat(&Tok::Assign) {
            let right = self.argument(false)?;
            Ok(self.push(Node::Bin(Bin::Sub, left, right)))
        } else {
            Ok(left)
        }
    }

    /// The second argument of a solve: one variable, `variable = value` as
    /// the initial value of a numeric solve, or a list of variables.
    fn solution_hint(&self, mut i: usize) -> Result<Hint, String> {
        let bad = || "expected up to three distinct solving variables".to_owned();
        let list = self.toks.get(i).is_some_and(|t| t.tok == Tok::LBrace);
        i += usize::from(list);
        let mut names = Vec::new();
        let mut axes = [0; 3];
        loop {
            let name = self.name_at(i).ok_or_else(bad)?;
            if names.len() == 3 || names.iter().any(|old| old == name) {
                return Err(bad());
            }
            axes[names.len()] = match name {
                "y" => 1,
                "z" => 2,
                _ => 0,
            };
            names.push(name.to_owned());
            i += 1;
            if !list || self.toks.get(i).is_none_or(|t| t.tok != Tok::Comma) {
                break;
            }
            i += 1;
        }
        if list {
            if self.toks.get(i).is_none_or(|t| t.tok != Tok::RBrace) {
                return Err(bad());
            }
            i += 1;
        }
        let guess = !list && self.toks.get(i).is_some_and(|t| t.tok == Tok::Assign);
        if !guess && self.toks.get(i).is_none_or(|t| t.tok != Tok::RParen) {
            return Err(bad());
        }
        let width = names.len() as u8;
        if !list && matches!(names[0].as_str(), "x" | "y" | "z") {
            // One of the coordinate names keeps its own axis, so that the
            // other names of the equation stay the other variables.
            names = vec!["x".into(), "y".into(), "z".into()];
        }
        Ok(Hint {
            names,
            width,
            axes,
            list,
            guess,
        })
    }
}

struct Hint {
    names: Vec<String>,
    width: u8,
    axes: [u8; 3],
    list: bool,
    guess: bool,
}
