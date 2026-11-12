use crate::{Error, Result, layout, models::*};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) const MAX_REVISION: u64 = 9_007_199_254_740_991;
pub(crate) const MAX_TIMESTAMP: u64 = 8_640_000_000_000_000;

/// A request that can be checked against the contract.
pub trait Validate {
    /// Checks the request, or answers `INVALID_INPUT` or `PAYLOAD_TOO_LARGE`.
    fn validate(&self) -> Result<()>;
}

pub(crate) fn parse<T: DeserializeOwned + Validate>(value: Value) -> Result<T> {
    let input: T = serde_json::from_value(value).map_err(|_| Error::invalid())?;
    input.validate()?;
    Ok(input)
}

/// A JSON request of `command` as its typed input, checked against the contract: `start`, `update` and `end` of the
/// templates, and `start_layout`, `update_layout` and `end_layout`, which are also read strictly, as activitykit
/// does. A framework that receives requests as JSON parses them with this.
pub fn parse_request<T: DeserializeOwned + Validate>(command: &str, input: Value) -> Result<T> {
    match command {
        "start_layout" | "update_layout" | "end_layout" => layout::parse(command, input),
        _ => parse(input),
    }
}

/// Validate a JSON request without invoking any native API: `start`, `update` and `end` of contract v1, and
/// `start_layout`, `update_layout` and `end_layout` of contract v2, which activitykit checks.
pub fn validate_request(command: &str, input: Value) -> Result<()> {
    match command {
        "start" => parse::<StartInput>(input).map(|_| ()),
        "update" => parse::<UpdateInput>(input).map(|_| ()),
        "end" => parse::<EndInput>(input).map(|_| ()),
        "start_layout" => layout::parse::<layout::LayoutStartInput>(command, input).map(|_| ()),
        "update_layout" => layout::parse::<layout::LayoutUpdateInput>(command, input).map(|_| ()),
        "end_layout" => layout::parse::<layout::LayoutEndInput>(command, input).map(|_| ()),
        _ => Err(Error::invalid()),
    }
}

pub(crate) fn text(value: &str, max: usize) -> bool {
    // Match JSON Schema's ECMAScript \S, including BOM, excluding NEL.
    let blank = |c: char| matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}');
    value.chars().count() <= max && value.chars().any(|c| !blank(c))
}

pub(crate) fn require(valid: bool) -> Result<()> {
    if valid { Ok(()) } else { Err(Error::invalid()) }
}

impl Validate for Snapshot {
    fn validate(&self) -> Result<()> {
        require(
            self.revision <= MAX_REVISION && text(&self.title, 120) && text(&self.status, 240),
        )?;
        require(self.stale_at.is_none_or(|value| value <= MAX_TIMESTAMP))?;
        match (&self.progress, &self.timer) {
            (Some(value), None) => require(value.is_finite() && (0.0..=1.0).contains(value)),
            (None, Some(timer)) => {
                require(timer.started_at < timer.ends_at && timer.ends_at <= MAX_TIMESTAMP)
            }
            _ => Err(Error::invalid()),
        }
    }
}

impl Validate for StartInput {
    fn validate(&self) -> Result<()> {
        let key = self.activity_key.as_bytes();
        require(
            !key.is_empty()
                && key.len() <= 128
                && key[0].is_ascii_alphanumeric()
                && key
                    .iter()
                    .all(|c| c.is_ascii_alphanumeric() || b"._:-".contains(c)),
        )?;
        require(self.schema_version == 1)?;
        self.snapshot.validate()?;
        require(matches!(
            (&self.template, &self.snapshot.progress),
            (Template::Progress, Some(_)) | (Template::Timer, None)
        ))
    }
}

impl Validate for UpdateInput {
    fn validate(&self) -> Result<()> {
        require(text(&self.id, 256))?;
        self.snapshot.validate()
    }
}

impl Validate for EndInput {
    fn validate(&self) -> Result<()> {
        require(text(&self.id, 256))?;
        match self.policy {
            EndPolicy::After { at } => require(at <= MAX_TIMESTAMP),
            _ => Ok(()),
        }
    }
}
