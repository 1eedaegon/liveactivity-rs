//! What pushes bring on iOS: a push token of an activity, the app's push-to-start token, and content a push
//! delivered. A backend hands each to the event sink as it comes, and keeps the newest tokens for `push_tokens`,
//! because a sink that listens late has missed the events. Android has no pushes.

use crate::layout::LayoutActivityRecord;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, PoisonError};

/// How many activities' tokens a backend keeps, the newest.
pub const TOKENS_KEPT: usize = 32;

/// A push token of an activity, new or changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushTokenEvent {
    /// The handle of the activity.
    pub activity_id: String,
    /// Absent when the backend no longer knows that activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
    /// The token, in lowercase hex.
    pub token: String,
}

/// The app's push-to-start token, new or changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushToStartTokenEvent {
    /// The token, in lowercase hex.
    pub token: String,
}

/// Content that a push delivered to a layout activity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedEvent {
    /// The activity as it is now.
    pub record: LayoutActivityRecord,
}

/// The push tokens a backend holds since launch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushTokens {
    /// The app's push-to-start token, when the backend watches it and has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_to_start: Option<String>,
    /// In the order the activities' first tokens came.
    pub activities: Vec<PushTokenEvent>,
}

/// A backend's record of the tokens, written before each event goes out; clones share it.
#[derive(Clone, Default)]
pub(crate) struct TokenBook(Arc<Mutex<PushTokens>>);

impl TokenBook {
    /// A token replaces the activity's; the oldest activity gives way to a new one past [`TOKENS_KEPT`].
    pub(crate) fn note_token(&self, token: &PushTokenEvent) {
        let mut tokens = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let activities = &mut tokens.activities;
        match activities
            .iter_mut()
            .find(|held| held.activity_id == token.activity_id)
        {
            Some(held) => *held = token.clone(),
            None => {
                if activities.len() == TOKENS_KEPT {
                    activities.remove(0);
                }
                activities.push(token.clone());
            }
        }
    }

    pub(crate) fn note_push_to_start(&self, token: &str) {
        let mut tokens = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        tokens.push_to_start = Some(token.to_owned());
    }

    pub(crate) fn tokens(&self) -> PushTokens {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(activity: usize, token: &str) -> PushTokenEvent {
        PushTokenEvent {
            activity_id: format!("la-{activity}"),
            activity_key: None,
            token: token.into(),
        }
    }

    #[test]
    fn the_book_keeps_the_newest_token_of_each_of_the_last_activities() {
        let book = TokenBook::default();
        assert_eq!(book.tokens(), PushTokens::default());
        book.note_push_to_start("aa");
        book.note_push_to_start("bb");
        for activity in 0..=TOKENS_KEPT {
            book.note_token(&token(activity, "01"));
        }
        book.note_token(&token(5, "02"));

        let tokens = book.clone().tokens();
        assert_eq!(tokens.push_to_start.as_deref(), Some("bb"));
        assert_eq!(tokens.activities.len(), TOKENS_KEPT);
        // The first activity gave way; a changed token keeps its place.
        assert_eq!(tokens.activities[0], token(1, "01"));
        assert_eq!(tokens.activities[4], token(5, "02"));
    }
}
