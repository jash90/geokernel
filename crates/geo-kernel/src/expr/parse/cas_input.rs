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
            [a] if op == Op::Simplify && !x && !y && self.is_text(a) => {
                // `CmdSimplify`: a text is tidied as a text.
                crate::expr::commands::text_call(self, crate::expr::TextOp::Simplify, &[a])
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
        let commas = self.argument_commas();
        // The Giac translation prints the second argument from its text, and
        // takes forms the kernel's own solver refuses (`x = 1`, a number).
        #[cfg(feature = "giac")]
        let hint = match commas[..] {
            [comma] => self.hint_text(comma + 1),
            _ => None,
        };
        let (names, width, axes) = if let [comma] = commas[..] {
            #[cfg(feature = "giac")]
            let variables = self.solution_variables(comma + 1).or_else(|_| {
                Ok::<_, String>((vec!["x".into(), "y".into(), "z".into()], 0, [0, 1, 2]))
            })?;
            #[cfg(not(feature = "giac"))]
            let variables = self.solution_variables(comma + 1)?;
            variables
        } else if commas.is_empty() {
            (vec!["x".into(), "y".into(), "z".into()], 0, [0, 1, 2])
        } else {
            return Err(format!("invalid arguments to `{name}`"));
        };
        let (outer_x, outer_y) = (self.uses_var, self.uses_y);
        let old = std::mem::replace(&mut self.cas_variables, names);
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
        if let [comma] = commas[..] {
            if self.pos != comma {
                return Err("expected a list of solving variables".into());
            }
            // The suffix was parsed and validated when choosing the variable scope.
            self.pos = self.toks[comma + 1..]
                .iter()
                .position(|t| t.tok == Tok::RParen)
                .map_or(self.toks.len(), |offset| comma + 1 + offset);
        }
        self.expect(&Tok::RParen, ")")?;
        self.cas_variables = old;
        self.split_names = old_split;
        (self.uses_var, self.uses_y) = (outer_x, outer_y);
        if equations.len() > 8 {
            return Err("at most eight polynomial equations can be solved together".into());
        }
        let is_list = is_list || matches!(equations.as_slice(), [arg] if self.is_list(*arg));
        let equations = self.wrap_list(&equations);
        let op = Op::Solve {
            assignments: matches!(name, "Solve" | "NSolve"),
            approximate: matches!(name, "NSolve" | "NSolutions"),
            width,
            axes,
            system: is_list,
        };
        let node = self.push(Node::Cas(op, equations, NONE));
        #[cfg(feature = "giac")]
        if let Some(hint) = hint {
            self.mark_hint(node, hint);
        }
        Ok(node)
    }

    fn cas_equation(&mut self) -> Result<u32, String> {
        let left = self.argument(false)?;
        if self.eat(&Tok::Assign) {
            let right = self.argument(false)?;
            let node = self.push(Node::Bin(Bin::Sub, left, right));
            // The Giac translation prints `left=right`, as the reference does.
            #[cfg(feature = "giac")]
            self.mark_equation(node);
            Ok(node)
        } else {
            Ok(left)
        }
    }

    fn solution_variables(&self, mut i: usize) -> Result<(Vec<String>, u8, [u8; 3]), String> {
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
        if self.toks.get(i).is_none_or(|t| t.tok != Tok::RParen) {
            return Err(bad());
        }
        let width = names.len() as u8;
        Ok((names, width, axes))
    }
}
