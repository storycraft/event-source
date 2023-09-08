# EventSource
Zero cost non buffered async event emitter

This crate is no_std

## Example
```rust ignore
async fn main() {
    let source: Arc<EventSource!(&mut i32)> = Arc::new(EventSource::new());

    // imaginary function for spawning future in another thread
    spawn({
        let source = source.clone();
        async {
            let mut a = 35;
            emit!(source, &mut a);
        }
    });

    let mut output = 0;
    // Closure can contain reference!
    source.on(|value, mut flow| {
        println!("Event emiited with value: {}!", value);
        output = *value;

        // mark listener as finished
        flow.set_done();
    }).await;

    println!("Output: {}", output);
}
```
This code outputs `Output: 35`

## License
MIT
