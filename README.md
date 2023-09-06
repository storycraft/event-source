# EventSource
Zero cost non buffered async event emitter

## Example
```rust
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
    source.on(|value| {
        println!("Event emiited with value: {}!", value);
        output = *value;

        Some(())
    }).await;

    println!("Output: {}", output);
}
```
This code outputs `Output: 35`

## License
MIT
