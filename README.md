# bevy-jev

A small, native [Bevy](https://bevyengine.org/) plugin for asynchronous, auditable Jev decisions. It keeps network work off the main schedule and accepts only responses that match the captured request, world revision, pack, model, and finite outcome allowlist. Requires Rust 1.95 or newer.

```rust
app.add_plugins(JevPlugin::new(GatewayProvider::new(
    "https://your-gateway.example/v1/decision",
    short_lived_session_token,
)?));

queue.submit(npc, JevRequest::new(
    "npc-42:9", "world:9", "npc/encounter",
    serde_json::json!({ "distance": 8 }),
    ["approach", "wait"],
))?;
```

The plugin inserts `JevDecisionPending` while a request is in flight, then replaces it with `JevDecisionOutcome`. If the entity disappears or its pending revision changes, the late response is ignored.

## Why a gateway?

Never ship a TypeSafe API key in a game binary. `GatewayProvider` expects a short-lived runtime token and requires HTTPS outside loopback development. The gateway owns secrets, rate limits, pack resolution, and the exact upstream Jev contract.

## Validate locally

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo run --example minimal
cargo run --example rejected_outcome
```

This repository is tested against Bevy 0.19.1. It does not call the live Jev API in tests.

## See an invented outcome fail closed

`cargo run --example rejected_outcome` uses a synthetic provider that returns `delete-save` while the NPC only allows `approach` or `wait`. The ECS records a rejected decision and performs no game action. The example needs no gateway or TypeSafe key; it does not exercise a rendered Bevy game.

## Status

Early, intentionally narrow integration. The ECS scheduling, background provider, gateway transport, and provenance guards are implemented and unit-tested. A full rendered Bevy game was not launched in this development environment.

MIT — see [LICENSE](LICENSE).
