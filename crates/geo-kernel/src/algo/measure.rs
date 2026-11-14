//! Numbers measured on objects: length, radius, slope, area.

use super::{Group, Parents};
use crate::kernel::{Construction, Error, Kind, Value};

/// Numbers measured on objects: length, radius, slope, area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureDef {}

impl Group for MeasureDef {
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
