//! One activity from start to end. `main` names the backend of the testkit, so the example runs anywhere; an
//! application leaves `.backend(..)` out and gets the backend of its platform.

use liveactivity::{EndInput, Event, LiveActivity, Result, StartInput, UpdateInput};

async fn deliver(live: &LiveActivity) -> Result<()> {
    // The key is the application's own name of the activity. Starting again with the same key and content answers
    // the activity that already runs.
    let order = live
        .start(StartInput::progress(
            "order:42",
            "Order 42",
            "Preparing",
            0.1,
        ))
        .await?;

    // An update sends the whole content again under a higher revision.
    live.update(UpdateInput::progress(
        &order.id,
        1,
        "Order 42",
        "On the way",
        0.6,
    ))
    .await?;

    // Android queues taps: ask for them when the application resumes. Elsewhere this does nothing.
    live.sync_events().await?;

    live.end(EndInput::new(&order.id)).await?;
    Ok(())
}

fn main() -> Result<()> {
    use liveactivity::testkit::MemoryBackend;

    // The sink gets what the platform reports: taps on an activity's buttons, and on iOS what pushes bring. Any
    // closure is one.
    let sink = |event: Event| {
        if let Event::Action(tap) = event {
            println!("{} tapped {}", tap.activity_id, tap.action_id);
        }
    };
    let live = LiveActivity::builder()
        .backend(MemoryBackend::new())
        .event_sink(sink)
        .build()?;

    liveactivity::block_on(deliver(&live))?;
    println!("{:?}", liveactivity::block_on(live.list())?[0].state);
    Ok(())
}
