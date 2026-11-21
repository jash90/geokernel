// Parts of this file are translated from GeoGebra (https://github.com/geogebra/geogebra),
// commit 112919f2d450ec7430a5cd3b65dba226fdc43b74:
//   source/shared/common/src/main/java/org/geogebra/common/kernel/parser/function/ParserFunctionsFactory.java
// Copyright (c) GeoGebra GmbH, Altenbergerstr. 69, 4040 Linz, Austria.
// Licensed by GeoGebra GmbH under the EUPL 1.2.

//! Names of the built-in functions.

use super::{commands, Bin, Un};

/// Whether `name` is a built-in function of any number of arguments.
pub(crate) fn builtin_name(name: &str) -> bool {
    unary_builtin(name).is_some() || binary_builtin(name).is_some()
}

/// Built-in functions and the coordinate functions `x` and `y`.
pub(crate) fn is_builtin(name: &str) -> bool {
    builtin_name(name) || name == "x" || name == "y"
}

/// One-argument functions of `ParserFunctionsFactory`.
pub(crate) fn unary_builtin(name: &str) -> Option<Un> {
    Some(match name {
        "sin" => Un::Sin,
        "cos" => Un::Cos,
        "tan" => Un::Tan,
        "csc" | "cosec" => Un::Csc,
        "sec" => Un::Sec,
        "cot" | "cotan" => Un::Cot,
        "sinh" => Un::Sinh,
        "cosh" => Un::Cosh,
        "tanh" => Un::Tanh,
        "csch" | "cosech" => Un::Csch,
        "sech" => Un::Sech,
        "coth" | "cotanh" => Un::Coth,
        "asin" | "arcsin" => Un::Asin,
        "acos" | "arccos" => Un::Acos,
        "atan" | "arctan" => Un::Atan,
        "asinh" | "arsinh" | "arcsinh" => Un::Asinh,
        "acosh" | "arcosh" | "arccosh" => Un::Acosh,
        "atanh" | "artanh" | "arctanh" => Un::Atanh,
        "sqrt" => Un::Sqrt,
        "cbrt" => Un::Cbrt,
        "abs" => Un::Abs,
        "sgn" | "sign" => Un::Sgn,
        "floor" => Un::Floor,
        "ceil" => Un::Ceil,
        "round" => Un::Round,
        "exp" => Un::Exp,
        "ln" => Un::Ln,
        "log" | "lg" | "log10" => Un::Log10,
        "ld" | "log2" => Un::Log2,
        "gamma" => Un::Gamma,
        _ => return None,
    })
}

/// Two-argument functions of `ParserFunctionsFactory`, and the commands of
/// two numbers that expressions support.
pub(crate) fn binary_builtin(name: &str) -> Option<Bin> {
    Some(match name {
        "atan2" | "arctan2" => Bin::Atan2,
        "log" | "ln" => Bin::LogB,
        "round" => Bin::Round2,
        // commands of two numbers, computed in place
        _ => return commands::binary_command(name),
    })
}
