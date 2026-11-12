//! Request contract v2: a layout tree, a style table and a map of values.
//!
//! The node and style types and the validator are the `activitykit` crate's, re-exported here under the names of 0.3.
//! The requests and records are this crate's own, so that their values stay a `BTreeMap` and Android reads the same
//! JSON; each converts into activitykit's request to be checked.

use crate::{
    Error, Result,
    models::{ActivityState, Backend, EndPolicy, Fallback, PromotionState, Visibility},
    validation::Validate,
};
use activitykit::contract as wire;
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;

pub use activitykit::Value;
pub use activitykit::contract::{
    AndroidOptions, AndroidVisibility, Island, IslandExpanded, weight,
};
pub use activitykit::layout::{
    Alignment, Bindable, Bound, ButtonStyle, Color, ColorName, ContentMode, Dimension, Element,
    Font, FontDesign, FontWeight, Frame, HorizontalAlignment, ImageSource, IndicatorKind, Node,
    Padding, Style, SymbolRendering, VerticalAlignment,
};

/// The member `layout` of a start: one tree per region and the two tints. The layout is fixed when the activity
/// starts; only a bound `background` or `keyline` follows the values.
pub type Layout = wire::LayoutRegions;

/// Reads a member that may be absent but never `null`.
fn non_null<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

fn none<T>(member: &Option<T>) -> bool {
    member.is_none()
}

/// The values a layout shows. An update replaces the whole map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutSnapshot {
    pub revision: u64,
    pub values: BTreeMap<String, Value>,
    /// When the system may show the activity as out of date, in epoch milliseconds.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub stale_at: Option<u64>,
    /// When the content was made, in epoch milliseconds. ActivityKit (iOS 17.2) drops content older than what it
    /// shows. Without it, iOS takes the time the content is sent.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutStartInput {
    pub activity_key: String,
    pub schema_version: u8,
    pub fallback: Fallback,
    pub layout: Layout,
    /// Styles by token; nodes name them with `style`.
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub styles: Option<BTreeMap<String, Style>>,
    pub snapshot: LayoutSnapshot,
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub android: Option<AndroidOptions>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutUpdateInput {
    pub id: String,
    pub snapshot: LayoutSnapshot,
}

/// The input of `end_layout`: the policy, and optionally the final content, which is checked as an update. Without
/// it the activity ends with its last accepted values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutEndInput {
    pub id: String,
    pub policy: EndPolicy,
    #[serde(default, deserialize_with = "non_null", skip_serializing_if = "none")]
    pub snapshot: Option<LayoutSnapshot>,
}

/// A layout activity as the native side reports it. It does not repeat the layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutActivityRecord {
    pub id: String,
    pub activity_key: String,
    pub schema_version: u8,
    pub snapshot: LayoutSnapshot,
    pub backend: Backend,
    pub state: ActivityState,
    pub visibility: Visibility,
    pub degraded_features: Vec<String>,
    pub promotion_requested: bool,
    pub promotion_state: PromotionState,
}

/// A tap on a button node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionEvent {
    pub action_id: String,
    /// Handle of the activity that showed the button.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
}

impl LayoutSnapshot {
    fn wire(&self) -> wire::LayoutSnapshot {
        wire::LayoutSnapshot {
            revision: self.revision,
            values: self.values.clone().into(),
            stale_at: self.stale_at,
            timestamp: self.timestamp,
        }
    }
}

impl LayoutStartInput {
    /// The same request in activitykit's types.
    pub(crate) fn wire(&self) -> wire::LayoutStartInput {
        wire::LayoutStartInput {
            activity_key: self.activity_key.clone(),
            schema_version: self.schema_version,
            fallback: match self.fallback {
                Fallback::None => wire::Fallback::None,
                Fallback::Notification => wire::Fallback::Notification,
            },
            layout: self.layout.clone(),
            styles: self.styles.clone(),
            snapshot: self.snapshot.wire(),
            android: self.android.clone(),
        }
    }

    /// Checks `update` against this start as the native layer does: the same keys and value types, the layout's
    /// references (bound style members included) and the weight of the layout, the styles and the new values.
    pub fn validate_update(&self, update: &LayoutUpdateInput) -> Result<()> {
        Ok(self.wire().validate_update(&update.wire())?)
    }

    /// Checks `end` against this start: its final content, when it has one, as an update.
    pub fn validate_end(&self, end: &LayoutEndInput) -> Result<()> {
        Ok(self.wire().validate_end(&end.wire())?)
    }
}

impl LayoutUpdateInput {
    fn wire(&self) -> wire::LayoutUpdateInput {
        wire::LayoutUpdateInput {
            id: self.id.clone(),
            snapshot: self.snapshot.wire(),
        }
    }
}

impl LayoutEndInput {
    fn wire(&self) -> wire::EndInput {
        wire::EndInput {
            id: self.id.clone(),
            policy: match self.policy {
                EndPolicy::Immediate {} => wire::EndPolicy::Immediate {},
                EndPolicy::Default {} => wire::EndPolicy::Default {},
                EndPolicy::After { at } => wire::EndPolicy::After { at },
            },
            snapshot: self.snapshot.as_ref().map(LayoutSnapshot::wire),
        }
    }
}

impl Validate for LayoutSnapshot {
    fn validate(&self) -> Result<()> {
        Ok(wire::Validate::validate(&self.wire())?)
    }
}

/// The value key that marks contract v1's template activities on iOS: a layout request may not use it, or its activity
/// would pass for a template and leave the layout commands.
pub(crate) const TEMPLATE_MARKER: &str = "liveActivityTemplate";

/// This crate's rule beyond activitykit's contract: no value under [`TEMPLATE_MARKER`].
fn unmarked(snapshot: &LayoutSnapshot) -> Result<()> {
    if snapshot.values.contains_key(TEMPLATE_MARKER) {
        Err(Error::invalid())
    } else {
        Ok(())
    }
}

impl LayoutStartInput {
    /// activitykit's rules alone, for the starts that the backend makes of contract v1's templates.
    pub(crate) fn validate_contract(&self) -> Result<()> {
        Ok(wire::Validate::validate(&self.wire())?)
    }
}

impl Validate for LayoutStartInput {
    fn validate(&self) -> Result<()> {
        self.validate_contract()?;
        unmarked(&self.snapshot)
    }
}

impl Validate for LayoutUpdateInput {
    fn validate(&self) -> Result<()> {
        wire::Validate::validate(&self.wire())?;
        unmarked(&self.snapshot)
    }
}

impl Validate for LayoutEndInput {
    fn validate(&self) -> Result<()> {
        wire::Validate::validate(&self.wire())?;
        self.snapshot.as_ref().map_or(Ok(()), unmarked)
    }
}

/// A JSON request of `command` (`start_layout`, `update_layout` or `end_layout`) as activitykit checks it, strictly
/// (unknown members, `null`, a request that does not read back as itself), and then in this crate's type, with the
/// crate's rules.
pub(crate) fn parse<T: DeserializeOwned + Validate>(
    command: &str,
    input: serde_json::Value,
) -> Result<T> {
    wire::validate_request(command, input.clone())?;
    let typed: T = serde_json::from_value(input).map_err(|_| Error::invalid())?;
    typed.validate()?;
    Ok(typed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn start() -> serde_json::Value {
        json!({"activityKey":"demo:1","schemaVersion":2,"fallback":"notification",
            "layout":{"lockScreen":{"type":"text","bind":"status","style":"s"},"keyline":{"bind":"tone"}},
            "styles":{"s":{"color":{"bind":"tone"},"font":"headline"}},
            "snapshot":{"revision":0,"values":{"status":"On time","tone":"green"},"staleAt":5,"timestamp":7},
            "android":{"visibility":"private"}})
    }

    #[test]
    fn a_request_reads_into_the_contracts_types_and_writes_back_the_same_json() {
        let input: LayoutStartInput = parse("start_layout", start()).unwrap();
        assert_eq!(serde_json::to_value(&input).unwrap(), start());
        assert_eq!(input.snapshot.values["tone"], Value::Text("green".into()));
        assert_eq!(input.snapshot.timestamp, Some(7));
        assert_eq!(serde_json::to_value(input.wire()).unwrap(), start());
        let end = json!({"id":"a","policy":{"kind":"after","at":3},"snapshot":{"revision":1,"values":{}}});
        let typed: LayoutEndInput = parse("end_layout", end.clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), end);
        assert_eq!(serde_json::to_value(typed.wire()).unwrap(), end);
        let plain: LayoutEndInput =
            parse("end_layout", json!({"id":"a","policy":{"kind":"default"}})).unwrap();
        assert_eq!(plain.snapshot, None);
        assert_eq!(
            serde_json::to_value(plain.wire()).unwrap(),
            json!({"id":"a","policy":{"kind":"default"}})
        );
        let immediate = LayoutEndInput {
            id: "a".into(),
            policy: EndPolicy::Immediate {},
            snapshot: None,
        };
        assert_eq!(
            serde_json::to_value(immediate.wire()).unwrap()["policy"]["kind"],
            "immediate"
        );
    }

    #[test]
    fn parse_is_activitykits_strict_check_and_the_types_validate_the_same() {
        let mut nulled = start();
        nulled["snapshot"]["timestamp"] = json!(null);
        assert_eq!(
            parse::<LayoutStartInput>("start_layout", nulled),
            Err(Error::invalid())
        );
        let mut unknown = start();
        unknown["extra"] = json!(1);
        assert_eq!(
            parse::<LayoutStartInput>("start_layout", unknown),
            Err(Error::invalid())
        );
        // Valid for activitykit, but not this type.
        assert_eq!(
            parse::<LayoutStartInput>("end_layout", json!({"id":"a","policy":{"kind":"default"}})),
            Err(Error::invalid())
        );
        let input: LayoutStartInput = serde_json::from_value(start()).unwrap();
        assert_eq!(input.validate(), Ok(()));
        assert_eq!(input.snapshot.validate(), Ok(()));
        let mut late = input.snapshot.clone();
        late.timestamp = Some(8_640_000_000_000_001);
        assert_eq!(late.validate(), Err(Error::invalid()));
        let update = LayoutUpdateInput {
            id: "a".into(),
            snapshot: input.snapshot.clone(),
        };
        assert_eq!(
            (update.validate(), input.validate_update(&update)),
            (Ok(()), Ok(()))
        );
        let mut crimson = update.clone();
        crimson
            .snapshot
            .values
            .insert("tone".into(), Value::Text("crimson".into()));
        assert_eq!(input.validate_update(&crimson), Err(Error::invalid()));
        let end = LayoutEndInput {
            id: "a".into(),
            policy: EndPolicy::Default {},
            snapshot: Some(crimson.snapshot),
        };
        assert_eq!(
            (end.validate(), input.validate_end(&end)),
            (Ok(()), Err(Error::invalid()))
        );
        let bare = LayoutEndInput {
            snapshot: None,
            ..end
        };
        assert_eq!(input.validate_end(&bare), Ok(()));
    }

    #[test]
    fn the_template_marker_is_no_value_key_of_a_layout_request() {
        let code = |result: Result<()>| result.map_err(|error| error.code);
        let invalid = Err("INVALID_INPUT".to_owned());
        let mut marked = start();
        marked["snapshot"]["values"]["liveActivityTemplate"] = json!("progress");
        // The commands' check, and the Rust API's.
        assert_eq!(
            code(crate::validate_request("start_layout", marked.clone())),
            invalid
        );
        let typed: LayoutStartInput = serde_json::from_value(marked).unwrap();
        assert_eq!(code(typed.validate()), invalid);
        let update =
            json!({"id":"a","snapshot":{"revision":1,"values":{"liveActivityTemplate":"timer"}}});
        assert_eq!(
            code(crate::validate_request("update_layout", update.clone())),
            invalid
        );
        let typed: LayoutUpdateInput = serde_json::from_value(update).unwrap();
        assert_eq!(code(typed.validate()), invalid);
        let end = json!({"id":"a","policy":{"kind":"immediate"},
            "snapshot":{"revision":1,"values":{"liveActivityTemplate":"timer"}}});
        assert_eq!(
            code(crate::validate_request("end_layout", end.clone())),
            invalid
        );
        let typed: LayoutEndInput = serde_json::from_value(end).unwrap();
        assert_eq!(code(typed.validate()), invalid);
        // Only that key: a longer one is a value like any other, and an end without content has none.
        let mut similar = start();
        similar["snapshot"]["values"]["liveActivityTemplates"] = json!("progress");
        assert_eq!(
            code(crate::validate_request("start_layout", similar)),
            Ok(())
        );
        let bare = json!({"id":"a","policy":{"kind":"immediate"}});
        assert_eq!(code(crate::validate_request("end_layout", bare)), Ok(()));
    }
}
