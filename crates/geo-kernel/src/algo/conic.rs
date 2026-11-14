//! Circles and their parts: circle through three points, centre, incircle.

use super::{Group, Parents};
use crate::kernel::{Construction, Error, Kind, Value};

/// Circles and their parts: circle through three points, centre, incircle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConicDef {}

impl Group for ConicDef {
    fn kind(&self) -> Kind {
        match *self {}
    }

    fn parents(&self) -> Parents {
        match *self {}
    }

    fn check(&self, _c: &Construction) -> Result<(), Error> {
        match *self {}
    }

    fn compute(&self, _c: &Construction) -> Option<Value> {
        match *self {}
    }

    fn parse(_c: &Construction, _func: &str, _args: &[&str]) -> Option<Result<Self, String>> {
        None
    }
}
