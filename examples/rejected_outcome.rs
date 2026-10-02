use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_jev::{
    JevDecisionOutcome, JevPlugin, JevProvider, JevRequest, JevRequestQueue, JevResponse,
    ProviderError,
};

struct InventedOutcomeProvider;
impl JevProvider for InventedOutcomeProvider {
    fn decide(&self, request: &JevRequest) -> Result<JevResponse, ProviderError> {
        Ok(serde_json::from_value(serde_json::json!({
            "requestId": request.request_id,
            "revision": request.revision,
            "record": {"schemaVersion": 1, "pack": {"name": request.pack_id}, "model": "synthetic-fixture", "outcome": "delete-save"}
        })).expect("synthetic response shape"))
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(JevPlugin::new(InventedOutcomeProvider));
    let npc = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<JevRequestQueue>()
        .submit(
            npc,
            JevRequest::new(
                "npc-demo:1",
                "world:1",
                "npc/encounter",
                serde_json::json!({"distance": 8}),
                ["approach", "wait"],
            ),
        )
        .expect("finite request");
    for _ in 0..1000 {
        app.update();
        if let Some(outcome) = app.world().get::<JevDecisionOutcome>(npc) {
            assert!(matches!(outcome, JevDecisionOutcome::Rejected { .. }));
            println!("{outcome:?}");
            return;
        }
        std::thread::yield_now();
    }
    panic!("synthetic worker did not return");
}
