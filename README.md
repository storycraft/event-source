# EventSource
Zero cost non buffered async event emitter

This crate is no_std

## Features
1. Non buffered, immediate event dispatch
2. Zero cost listener adding, removing 
3. Higher kinded event type
4. Propagation control

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
    source.on(|value, flow| {
        println!("Event emiited with value: {}!", value);
        output = *value;

        // mark listener as finished
        flow.set_done();
    }).await;

    println!("Output: {}", output);
}
```

Output
```text
Event emiited with value: 35!
Output: 35
```

## License
MIT
