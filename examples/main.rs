use std::{sync::Arc, time::Duration};

use event_source::{emit, EventSource};
use tokio::{spawn, time::sleep};

fn spawn_emit_task(source: &Arc<EventSource!(&mut i32)>, value: i32, interval: Duration) {
    spawn({
        let source = source.clone();

        async move {
            let mut value = value;

            loop {
                emit!(source, &mut value);

                sleep(interval).await;
            }
        }
    });
}

#[tokio::main]
async fn main() {
    let source: Arc<EventSource!(&mut i32)> = Arc::new(EventSource::new());

    spawn_emit_task(&source, 5, Duration::from_millis(300));
    spawn_emit_task(&source, 10, Duration::from_millis(1000));

    source
        .on(|value, _| {
            dbg!(value);
        })
        .await;
}
