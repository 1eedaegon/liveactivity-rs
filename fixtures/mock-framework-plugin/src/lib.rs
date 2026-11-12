//! Independent fixture crate proving that a framework plugin can implement the
//! liveactivity-core service-provider interfaces.

#![forbid(unsafe_code)]

use liveactivity_core::{
    ActivityRecord, ActivityState, BackendContext, BackendFuture, CONTRACT_VERSION, Capabilities,
    EndRequest, Error, ErrorCode, EventSink, LiveActivity, LiveActivityBackend, LiveActivityEvent,
    Result, StartRequest, UpdateRequest,
};
use std::sync::mpsc::{self, Receiver, Sender};

/// A command observed behind the mock framework bridge.
#[derive(Debug, Clone, PartialEq)]
pub enum ObservedCall {
    /// Capability query.
    Capabilities,
    /// Start command.
    Start(StartRequest),
    /// Update command.
    Update(UpdateRequest),
    /// End command.
    End(EndRequest),
    /// List command.
    List,
}

/// Framework-side reverse bridge backed by a standard channel.
pub struct FrameworkEventSink {
    sender: Sender<LiveActivityEvent>,
}

impl FrameworkEventSink {
    /// Creates a sink that forwards events into the supplied channel.
    pub fn new(sender: Sender<LiveActivityEvent>) -> Self {
        Self { sender }
    }
}

impl EventSink for FrameworkEventSink {
    fn emit(&self, event: LiveActivityEvent) -> Result<()> {
        self.sender.send(event).map_err(|_| {
            Error::new(
                ErrorCode::EventDelivery,
                "the mock framework event receiver is closed",
            )
        })
    }
}

/// Backend implementation owned by an external framework-plugin crate.
pub struct MockFrameworkPlugin {
    context: BackendContext,
    calls: Sender<ObservedCall>,
    backend_name: String,
    start_error: Option<Error>,
}

impl MockFrameworkPlugin {
    /// Creates the plugin from the context supplied by LiveActivity::connect.
    pub fn new(
        context: BackendContext,
        calls: Sender<ObservedCall>,
        backend_name: impl Into<String>,
        start_error: Option<Error>,
    ) -> Self {
        Self {
            context,
            calls,
            backend_name: backend_name.into(),
            start_error,
        }
    }

    fn observe(&self, call: ObservedCall) -> Result<()> {
        self.calls.send(call).map_err(|_| {
            Error::new(
                ErrorCode::BackendFailure,
                "the mock command observer is closed",
            )
        })
    }

    fn capabilities_value(backend: String) -> Capabilities {
        Capabilities {
            contract_version: CONTRACT_VERSION,
            backend,
            start: true,
            update: true,
            end: true,
            list: true,
            actions: true,
        }
    }

    fn activity(
        backend: String,
        id: String,
        activity_key: String,
        revision: u64,
        state: ActivityState,
        content: serde_json::Value,
    ) -> ActivityRecord {
        ActivityRecord {
            contract_version: CONTRACT_VERSION,
            id,
            activity_key,
            revision,
            state,
            content,
            backend,
        }
    }
}

impl LiveActivityBackend for MockFrameworkPlugin {
    fn capabilities(&self) -> BackendFuture<'_, Capabilities> {
        let observed = self.observe(ObservedCall::Capabilities);
        let backend = self.backend_name.clone();
        Box::pin(async move {
            observed?;
            Ok(Self::capabilities_value(backend))
        })
    }

    fn start(&self, request: StartRequest) -> BackendFuture<'_, ActivityRecord> {
        let observed = self.observe(ObservedCall::Start(request.clone()));
        let start_error = self.start_error.clone();
        let context = self.context.clone();
        let backend = self.backend_name.clone();
        Box::pin(async move {
            observed?;
            if let Some(error) = start_error {
                return Err(error);
            }
            let activity = Self::activity(
                backend,
                format!("{}-id", request.activity_key),
                request.activity_key,
                0,
                ActivityState::Active,
                request.content,
            );
            context.emit(LiveActivityEvent::state_changed(activity.clone()))?;
            Ok(activity)
        })
    }

    fn update(&self, request: UpdateRequest) -> BackendFuture<'_, ActivityRecord> {
        let observed = self.observe(ObservedCall::Update(request.clone()));
        let backend = self.backend_name.clone();
        Box::pin(async move {
            observed?;
            Ok(Self::activity(
                backend,
                request.id,
                "recovered".into(),
                request.revision,
                ActivityState::Active,
                request.content,
            ))
        })
    }

    fn end(&self, request: EndRequest) -> BackendFuture<'_, ActivityRecord> {
        let observed = self.observe(ObservedCall::End(request.clone()));
        let backend = self.backend_name.clone();
        Box::pin(async move {
            observed?;
            Ok(Self::activity(
                backend,
                request.id,
                "recovered".into(),
                request.revision,
                ActivityState::Ended,
                request.content.unwrap_or(serde_json::Value::Null),
            ))
        })
    }

    fn list(&self) -> BackendFuture<'_, Vec<ActivityRecord>> {
        let observed = self.observe(ObservedCall::List);
        Box::pin(async move {
            observed?;
            Ok(Vec::new())
        })
    }
}

/// Connected service and observation channels used by conformance tests.
pub struct MockPluginHarness {
    /// Framework-neutral service under test.
    pub service: LiveActivity,
    /// Commands observed by the plugin backend.
    pub calls: Receiver<ObservedCall>,
    /// Events observed by the framework.
    pub events: Receiver<LiveActivityEvent>,
}

/// Connects the external mock plugin through only liveactivity-core public APIs.
pub fn connect_mock_plugin(
    backend_name: impl Into<String>,
    start_error: Option<Error>,
) -> Result<MockPluginHarness> {
    let (call_sender, calls) = mpsc::channel();
    let (event_sender, events) = mpsc::channel();
    let backend_name = backend_name.into();
    let service = LiveActivity::connect(FrameworkEventSink::new(event_sender), move |context| {
        Ok(MockFrameworkPlugin::new(
            context,
            call_sender,
            backend_name,
            start_error,
        ))
    })?;
    Ok(MockPluginHarness {
        service,
        calls,
        events,
    })
}
