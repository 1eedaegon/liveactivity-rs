use crate::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Countdown interval as Unix timestamps in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timer {
    /// Start of the countdown.
    pub started_at: u64,
    /// End of the countdown, after `started_at`.
    pub ends_at: u64,
}

/// Which of the two portable templates a content is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TemplateKind {
    /// A title, a status and a progress bar.
    Progress,
    /// A title, a status and a countdown.
    Timer,
}

/// Portable content: a title, a status and either a progress or a countdown.
///
/// Every backend that reports `templates` draws it with its platform's own presentation, so
/// the same request runs on any of them. The kind is fixed at start: an update keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Template {
    /// Headline of the activity.
    pub title: String,
    /// Current status line.
    pub status: String,
    /// Progress from zero through one, for the progress template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    /// Countdown, for the timer template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<Timer>,
    /// When the platform may show the content as out of date, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_at: Option<u64>,
}

impl Template {
    /// A progress template.
    pub fn progress(title: impl Into<String>, status: impl Into<String>, progress: f64) -> Self {
        Self {
            title: title.into(),
            status: status.into(),
            progress: Some(progress),
            timer: None,
            stale_at: None,
        }
    }

    /// A timer template counting down from `started_at` to `ends_at`.
    pub fn timer(
        title: impl Into<String>,
        status: impl Into<String>,
        started_at: u64,
        ends_at: u64,
    ) -> Self {
        Self {
            title: title.into(),
            status: status.into(),
            progress: None,
            timer: Some(Timer {
                started_at,
                ends_at,
            }),
            stale_at: None,
        }
    }

    /// The same template, stale from `at`.
    #[must_use]
    pub fn stale_at(self, at: u64) -> Self {
        Self {
            stale_at: Some(at),
            ..self
        }
    }

    /// The kind of a valid template: it has exactly one of a progress and a timer.
    pub fn kind(&self) -> Result<TemplateKind> {
        match (self.progress, self.timer) {
            (Some(progress), None) if progress.is_finite() && (0.0..=1.0).contains(&progress) => {
                Ok(TemplateKind::Progress)
            }
            (None, Some(timer)) if timer.ends_at > timer.started_at => Ok(TemplateKind::Timer),
            (Some(_), None) => Err(invalid("progress must be between 0 and 1")),
            (None, Some(_)) => Err(invalid("timer must end after it starts")),
            _ => Err(invalid("a template has exactly one of progress and timer")),
        }
    }

    /// The limits both default engines share, so that a template one backend takes every
    /// backend takes.
    pub(crate) fn validate(&self) -> Result<()> {
        if !text_fits(&self.title, MAX_TITLE_CHARACTERS) {
            return Err(invalid("title must be 1 to 120 characters and not blank"));
        }
        if !text_fits(&self.status, MAX_STATUS_CHARACTERS) {
            return Err(invalid("status must be 1 to 240 characters and not blank"));
        }
        let latest = [self.stale_at, self.timer.map(|timer| timer.ends_at)];
        if latest.iter().flatten().any(|at| *at > MAX_TIMESTAMP) {
            return Err(invalid("a time is out of range"));
        }
        self.kind().map(|_| ())
    }
}

const MAX_TITLE_CHARACTERS: usize = 120;
const MAX_STATUS_CHARACTERS: usize = 240;
/// The latest time both default engines take: 9999-12-31T23:59:59.999Z in Unix milliseconds.
const MAX_TIMESTAMP: u64 = 253_402_300_799_999;

fn text_fits(text: &str, maximum: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= maximum
}

fn invalid(message: &'static str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}

/// What an activity shows.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Content {
    /// Portable content that every backend with `templates` draws.
    Template(Template),
    /// Content in the selected backend's own contract. It runs on that backend only.
    Native {
        /// The backend's content after framework-boundary validation.
        value: Value,
    },
}

impl Content {
    /// The backend-specific value, when the content is not portable.
    pub fn native(&self) -> Option<&Value> {
        match self {
            Self::Native { value } => Some(value),
            Self::Template(_) => None,
        }
    }

    /// The portable template, when the content is one.
    pub fn template(&self) -> Option<&Template> {
        match self {
            Self::Template(template) => Some(template),
            Self::Native { .. } => None,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Self::Template(template) => template.validate(),
            Self::Native { .. } => Ok(()),
        }
    }
}

impl From<Template> for Content {
    fn from(template: Template) -> Self {
        Self::Template(template)
    }
}

impl From<Value> for Content {
    fn from(value: Value) -> Self {
        Self::Native { value }
    }
}

/// What a start does when the platform cannot give the activity its native presentation,
/// such as a promoted Live Update.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Fallback {
    /// Fail with `unsupported` and show nothing.
    None,
    /// Show the activity in the platform's ordinary presentation.
    #[default]
    Notification,
}
