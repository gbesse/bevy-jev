//! Non-blocking Jev decisions for Bevy applications.
//!
//! Requests are executed by a dedicated worker, and every result is checked
//! against the request id, world revision, pack id, and finite outcome set.

mod provider;

use bevy_app::{App, Plugin, PreUpdate};
use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};

#[cfg(feature = "gateway")]
pub use provider::GatewayProvider;
pub use provider::{JevProvider, ProviderError};

/// A bounded decision request captured at a particular world revision.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JevRequest {
    pub request_id: String,
    pub revision: String,
    pub pack_id: String,
    pub state: Value,
    #[serde(skip)]
    pub allowed_outcomes: Vec<String>,
}

impl JevRequest {
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        revision: impl Into<String>,
        pack_id: impl Into<String>,
        state: Value,
        allowed_outcomes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            revision: revision.into(),
            pack_id: pack_id.into(),
            state,
            allowed_outcomes: allowed_outcomes.into_iter().map(Into::into).collect(),
        }
    }
}

/// The subset of a gateway response required to prove decision provenance.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JevResponse {
    pub request_id: String,
    pub revision: String,
    pub record: JevRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JevRecord {
    pub schema_version: u32,
    pub pack: JevPackRef,
    pub model: String,
    pub outcome: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JevPackRef {
    pub name: String,
}

/// Marks an entity while its decision is in flight.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct JevDecisionPending {
    pub request_id: String,
    pub revision: String,
}

/// Inserted on the entity once a verified decision completes.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum JevDecisionOutcome {
    Accepted { outcome: String, model: String },
    Rejected { reason: String },
}

#[derive(Debug)]
struct WorkItem {
    entity: Entity,
    request: JevRequest,
}

#[derive(Debug)]
struct WorkResult {
    entity: Entity,
    request: JevRequest,
    response: Result<JevResponse, ProviderError>,
}

/// Requests waiting to be sent to the worker.
#[derive(Resource, Default)]
pub struct JevRequestQueue(VecDeque<(Entity, JevRequest)>);

impl JevRequestQueue {
    /// Queues a request for the given entity.
    ///
    /// # Errors
    ///
    /// Returns an error when provenance fields are empty or fewer than two outcomes are declared.
    pub fn submit(&mut self, entity: Entity, request: JevRequest) -> Result<(), &'static str> {
        if request.request_id.is_empty()
            || request.revision.is_empty()
            || request.pack_id.is_empty()
        {
            return Err("request id, revision and pack id are required");
        }
        if request.allowed_outcomes.len() < 2 {
            return Err("at least two finite outcomes are required");
        }
        self.0.push_back((entity, request));
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Resource)]
struct JevWorker {
    send: mpsc::Sender<WorkItem>,
    receive: Arc<Mutex<mpsc::Receiver<WorkResult>>>,
}

/// Installs the request queue and result application systems.
pub struct JevPlugin {
    provider: Arc<dyn JevProvider>,
}

impl JevPlugin {
    #[must_use]
    pub fn new(provider: impl JevProvider) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }
}

impl Plugin for JevPlugin {
    fn build(&self, app: &mut App) {
        let (work_tx, work_rx) = mpsc::channel::<WorkItem>();
        let (result_tx, result_rx) = mpsc::channel::<WorkResult>();
        let provider = Arc::clone(&self.provider);
        std::thread::Builder::new()
            .name("bevy-jev-worker".into())
            .spawn(move || {
                while let Ok(work) = work_rx.recv() {
                    let response = provider.decide(&work.request);
                    if result_tx
                        .send(WorkResult {
                            entity: work.entity,
                            request: work.request,
                            response,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("failed to spawn Jev worker");

        app.init_resource::<JevRequestQueue>()
            .insert_resource(JevWorker {
                send: work_tx,
                receive: Arc::new(Mutex::new(result_rx)),
            })
            .add_systems(PreUpdate, (dispatch_requests, apply_results).chain());
    }
}

#[allow(clippy::needless_pass_by_value)] // Bevy system parameters are intentionally passed by value.
fn dispatch_requests(
    mut commands: Commands,
    mut queue: ResMut<JevRequestQueue>,
    worker: Res<JevWorker>,
) {
    while let Some((entity, request)) = queue.0.pop_front() {
        if let Ok(mut target) = commands.get_entity(entity) {
            target.insert(JevDecisionPending {
                request_id: request.request_id.clone(),
                revision: request.revision.clone(),
            });
            if worker.send.send(WorkItem { entity, request }).is_err() {
                target.insert(JevDecisionOutcome::Rejected {
                    reason: "Jev worker stopped".into(),
                });
                target.remove::<JevDecisionPending>();
            }
        }
    }
}

#[allow(clippy::needless_pass_by_value)] // Bevy system parameters are intentionally passed by value.
fn apply_results(
    mut commands: Commands,
    worker: Res<JevWorker>,
    pending: Query<&JevDecisionPending>,
) {
    let Ok(receiver) = worker.receive.lock() else {
        return;
    };
    for result in receiver.try_iter() {
        let Ok(current) = pending.get(result.entity) else {
            continue;
        };
        if current.request_id != result.request.request_id
            || current.revision != result.request.revision
        {
            continue;
        }
        let outcome = match result.response {
            Ok(response) => validate_response(&result.request, &response).map_or_else(
                |reason| JevDecisionOutcome::Rejected { reason },
                |(outcome, model)| JevDecisionOutcome::Accepted { outcome, model },
            ),
            Err(error) => JevDecisionOutcome::Rejected {
                reason: error.to_string(),
            },
        };
        if let Ok(mut target) = commands.get_entity(result.entity) {
            target.insert(outcome).remove::<JevDecisionPending>();
        }
    }
}

fn validate_response(
    request: &JevRequest,
    response: &JevResponse,
) -> Result<(String, String), String> {
    if response.request_id != request.request_id || response.revision != request.revision {
        return Err("decision request provenance mismatch".into());
    }
    if response.record.schema_version != 1
        || response.record.pack.name != request.pack_id
        || response.record.model.is_empty()
    {
        return Err("invalid decision record provenance".into());
    }
    if !request.allowed_outcomes.contains(&response.record.outcome) {
        return Err("decision outcome is not in the finite allowlist".into());
    }
    Ok((
        response.record.outcome.clone(),
        response.record.model.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    struct FakeProvider {
        response: JevResponse,
        delay: Duration,
    }
    impl JevProvider for FakeProvider {
        fn decide(&self, _: &JevRequest) -> Result<JevResponse, ProviderError> {
            std::thread::sleep(self.delay);
            Ok(self.response.clone())
        }
    }

    fn response(outcome: &str) -> JevResponse {
        JevResponse {
            request_id: "req-1".into(),
            revision: "world-7".into(),
            record: JevRecord {
                schema_version: 1,
                pack: JevPackRef {
                    name: "npc/combat".into(),
                },
                model: "jev-1.13.0".into(),
                outcome: outcome.into(),
            },
        }
    }

    fn run_until_done(app: &mut App, entity: Entity) -> JevDecisionOutcome {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            app.update();
            if let Some(outcome) = app.world().get::<JevDecisionOutcome>(entity) {
                return outcome.clone();
            }
            assert!(Instant::now() < deadline, "worker timed out");
            std::thread::yield_now();
        }
    }

    #[test]
    fn applies_a_proven_finite_outcome() {
        let mut app = App::new();
        app.add_plugins(JevPlugin::new(FakeProvider {
            response: response("attack"),
            delay: Duration::ZERO,
        }));
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<JevRequestQueue>()
            .submit(
                entity,
                JevRequest::new(
                    "req-1",
                    "world-7",
                    "npc/combat",
                    serde_json::json!({"distance": 3}),
                    ["attack", "retreat"],
                ),
            )
            .unwrap();
        assert_eq!(
            run_until_done(&mut app, entity),
            JevDecisionOutcome::Accepted {
                outcome: "attack".into(),
                model: "jev-1.13.0".into()
            }
        );
    }

    #[test]
    fn rejects_an_outcome_outside_the_allowlist() {
        let mut app = App::new();
        app.add_plugins(JevPlugin::new(FakeProvider {
            response: response("dance"),
            delay: Duration::ZERO,
        }));
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<JevRequestQueue>()
            .submit(
                entity,
                JevRequest::new(
                    "req-1",
                    "world-7",
                    "npc/combat",
                    Value::Null,
                    ["attack", "retreat"],
                ),
            )
            .unwrap();
        assert!(matches!(
            run_until_done(&mut app, entity),
            JevDecisionOutcome::Rejected { .. }
        ));
    }

    #[test]
    fn ignores_a_result_after_revision_changes() {
        let mut app = App::new();
        app.add_plugins(JevPlugin::new(FakeProvider {
            response: response("attack"),
            delay: Duration::from_millis(20),
        }));
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<JevRequestQueue>()
            .submit(
                entity,
                JevRequest::new(
                    "req-1",
                    "world-7",
                    "npc/combat",
                    Value::Null,
                    ["attack", "retreat"],
                ),
            )
            .unwrap();
        app.update();
        app.world_mut()
            .entity_mut(entity)
            .insert(JevDecisionPending {
                request_id: "req-1".into(),
                revision: "world-8".into(),
            });
        std::thread::sleep(Duration::from_millis(30));
        for _ in 0..5 {
            app.update();
            std::thread::yield_now();
        }
        assert!(app.world().get::<JevDecisionOutcome>(entity).is_none());
    }
}
