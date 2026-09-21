use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_jev::{
    JevDecisionOutcome, JevPlugin, JevProvider, JevRequest, JevRequestQueue, JevResponse,
    ProviderError,
};

struct DemoProvider;
impl JevProvider for DemoProvider {
    fn decide(&self, request: &JevRequest) -> Result<JevResponse, ProviderError> {
        Ok(serde_json::from_value(serde_json::json!({
            "requestId": request.request_id, "revision": request.revision,
            "record": {"schemaVersion": 1, "pack": {"name": request.pack_id}, "model": "jev-1.13.0", "outcome": "approach"}
        })).unwrap())
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(JevPlugin::new(DemoProvider));
    let npc = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<JevRequestQueue>()
        .submit(
            npc,
            JevRequest::new(
                "npc-42:9",
                "world:9",
                "npc/encounter",
                serde_json::json!({"distance": 8}),
                ["approach", "wait"],
            ),
        )
        .unwrap();
    for _ in 0..100 {
        app.update();
        if let Some(outcome) = app.world().get::<JevDecisionOutcome>(npc) {
            println!("{outcome:?}");
            break;
        }
        std::thread::yield_now();
    }
}
