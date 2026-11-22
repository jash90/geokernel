// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/commands/CmdSolve.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/CmdSimplify.java
//   source/shared/common/src/main/java/org/geogebra/common/kernel/cas/CmdCASCommand1Arg.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

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
                if op.is_function()
                    && self.checked_type(a)? == Ty::Object
                    && !(op == Op::Simplify && self.as_function(a))
                {
                    return Err(format!("invalid arguments to `{name}`"));
                }
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
        // What main parses keeps main's reading of the solving variables, so
        // that main's answers stay; the readings of `solution_hint` are for
        // the forms main refused (a start value, `PlotSolve`, an equation
        // main could not read with its variables).
        let legacy = match commas[..] {
            _ if points => None,
            [] => Some((vec!["x".into(), "y".into(), "z".into()], 0, [0, 1, 2])),
            [comma] => self.solution_variables(comma + 1).ok(),
            _ => None,
        };
        let Some(legacy) = legacy else {
            return self.parse_solve_as(name, &commas, None);
        };
        let saved = self.saved();
        let state = (self.uses_y, self.cas_variables.clone(), self.split_names);
        match self.parse_solve_as(name, &commas, Some(legacy)) {
            Ok(node) => Ok(node),
            Err(error) if commas.is_empty() => Err(error),
            Err(error) => {
                self.reset(saved);
                (self.uses_y, self.cas_variables, self.split_names) = state;
                self.parse_solve_as(name, &commas, None).map_err(|_| error)
            }
        }
    }

    fn parse_solve_as(
        &mut self,
        name: &str,
        commas: &[usize],
        legacy: Option<(Vec<String>, u8, [u8; 3])>,
    ) -> Result<u32, String> {
        let points = name == "PlotSolve";
        let is_legacy = legacy.is_some();
        let hint = match (legacy, commas) {
            (Some((names, width, axes)), _) => Hint {
                names,
                width,
                axes,
                list: false,
                guess: false,
            },
            (None, []) => Hint {
                names: vec!["x".into(), "y".into(), "z".into()],
                width: 0,
                axes: [0, 1, 2],
                list: false,
                guess: false,
            },
            // `CmdSolve.process`: the plotted solve takes one argument.
            (None, [comma]) if !points => self.solution_hint(*comma + 1, name)?,
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
        if let ([equation], true) = (&equations[..], points && !is_list) {
            // `CmdSolve.solve`: a list, an equation or a function; a number
            // is an illegal argument.
            let equation = *equation;
            let accepted = match self.checked_type(equation)? {
                Ty::Num(_) => self.reads_variable(equation),
                Ty::Function | Ty::List(_) => true,
                Ty::Object => {
                    self.as_function(equation)
                        || matches!(self.node(equation), Node::Obj(id) if self.c.kind(id) == crate::Kind::ImplicitCurve)
                }
                _ => false,
            };
            if !accepted {
                return Err(format!("invalid arguments to `{name}`"));
            }
        }
        let mut guess = NONE;
        if let [comma] = commas[..] {
            if self.pos != comma {
                return Err("expected a list of solving variables".into());
            }
            let listed = is_list || matches!(equations[..], [arg] if self.is_list(arg));
            if listed && !hint.list && hint.axes[0] == crate::expr::cas::NO_VARIABLE {
                // The reference prints the value of the number into its
                // system solver and reads garbage back.
                return Err(format!("invalid arguments to `{name}`"));
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
            legacy: is_legacy,
        };
        Ok(self.push(Node::Cas(op, equations, guess)))
    }

    /// Whether the node is an object `CmdSimplify` reads as a function of
    /// `x`: a line (`GeoLine` and its segment and ray) or a conic
    /// (`GeoFunctionable`).
    fn as_function(&self, i: u32) -> bool {
        matches!(self.node(i), Node::Obj(id) if matches!(
            self.c.kind(id),
            crate::Kind::Line
                | crate::Kind::Segment
                | crate::Kind::Ray
                | crate::Kind::Conic
                | crate::Kind::Circle
        ))
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

    /// Whether the subtree holds a variable or a function, so that it is
    /// more than a number.
    fn reads_variable(&self, root: u32) -> bool {
        let mut pending = vec![root];
        while let Some(i) = pending.pop() {
            let node = self.node(i);
            if matches!(
                node,
                Node::Var
                    | Node::VarY
                    | Node::CasVariable(_)
                    | Node::FunctionObject(_)
                    | Node::Apply(..)
                    | Node::Apply2(..)
            ) {
                return true;
            }
            pending.extend(crate::expr::children(&node));
        }
        false
    }

    /// The solving variables as main reads them: one name or a list of up
    /// to three distinct names, `y` and `z` on their own axes and any other
    /// name on the first.
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

    /// The second argument of a solve: one variable, `variable = value` as
    /// the initial value of a numeric solve, or a list of variables.
    fn solution_hint(&self, mut i: usize, command: &str) -> Result<Hint, String> {
        let bad = || "expected up to three distinct solving variables".to_owned();
        let list = self.toks.get(i).is_some_and(|t| t.tok == Tok::LBrace);
        i += usize::from(list);
        let first = i;
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
        if list && names == ["t"] {
            // `t` is a variable of its own beside `x` and `y`, so that
            // `Solutions(f, {t})` of a function of `x` solves for a
            // variable the function does not hold.
            names = vec!["x".into(), "y".into(), "t".into()];
            axes[0] = 2;
        }
        if !list {
            if guess && command == "Solve" && names[0] != "z" && self.object_at(first).is_none() {
                // `Solve` takes `name = value` for an assumption and solves
                // for `x`, whatever the name is. An assumption about the
                // other variable is marked in the unused second axis.
                if names[0] != "x" {
                    axes[1] = crate::expr::cas::NO_VARIABLE;
                }
                names = vec!["x".into(), "y".into(), "z".into()];
                axes[0] = 0;
            } else if matches!(names[0].as_str(), "x" | "y" | "z") {
                // A coordinate name keeps its own axis, so that the other
                // names of the equation stay the other variables.
                names = vec!["x".into(), "y".into(), "z".into()];
            } else if let Some(id) = self.object_at(first) {
                // `AlgoSolve.printHint` prints the value of an object: a
                // number is no variable to solve for, and `NSolve` then
                // reads an answer back that the reference cannot parse.
                // What the CAS makes of any other object follows no rule.
                let number = matches!(self.c.kind(id), crate::Kind::Number | crate::Kind::Angle);
                if !number || (command == "NSolve" && !guess) {
                    return Err(format!("invalid arguments to `{command}`"));
                }
                names = vec!["x".into(), "y".into(), "z".into()];
                axes[0] = crate::expr::cas::NO_VARIABLE;
            } else if names[0] == "t" {
                // The parameter name is a variable of its own, beside `x`
                // and `y`.
                names = vec!["x".into(), "y".into(), "t".into()];
                axes[0] = 2;
            } else {
                // The reference knows no other free name.
                return Err(format!("undefined variable `{}`", names[0]));
            }
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
