# FX quote timeout example

This example runs two Summer applications. The FX provider serves real HTTP JSON responses: the
USD/THB quote responds in 10 milliseconds and the USD/SGD quote responds in 100 milliseconds. The
client applies a 50 millisecond `#[timeout]`, so the fast HTTP request succeeds and the slow request
returns a typed `TimeoutElapsed` error.

Run both commands from this directory so each application loads `config/app.toml`.

Start the provider:

```bash
cargo run --bin fx-provider
```

Then run the client in another terminal:

```bash
cargo run --bin timeout-client
```
