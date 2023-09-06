use std::{sync::Arc, time::Duration};

use event_source::{emit, EventSource};
use tokio::{runtime::Builder, spawn, time::sleep};

fn main() {
    Builder::new_multi_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(async_main());
}

async fn async_main() {
    let source: Arc<EventSource!(&mut i32)> = Arc::new(EventSource::new());

    spawn({
        let source = source.clone();

        async move {
            let mut a = 5;

            loop {
                emit!(source, &mut a);

                sleep(Duration::from_secs(1)).await;
            }
        }
    });

    spawn({
        let source = source.clone();

        async move {
            let mut a = 19;

            loop {
                emit!(source, &mut a);

                sleep(Duration::from_secs(2)).await;

                let _ = source
                    .on(|a| {
                        dbg!(a);
                        Some(())
                    })
                    .await;
            }
        }
    });

    let _ = source
        .on(|a| {
            dbg!(a);
            None
        })
        .await;
}
