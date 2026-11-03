pub mod peers;

#[path = "../../../server/src/serve/launch_phase.rs"]
pub mod launch_phase;
#[path = "../../../server/src/serve/launch_reply.rs"]
pub mod launch_reply;
#[path = "../../../server/src/serve/return_reply.rs"]
pub mod return_reply;
#[path = "../../../server/src/serve/launch.rs"]
pub mod launch;
#[path = "../../../server/src/serve/returned.rs"]
pub mod returned;
#[path = "../../../server/src/serve/execution.rs"]
pub mod execution;

#[path = "../../../server/src/serve/binding_rpc.rs"]
mod binding_rpc;
#[path = "../../../server/src/serve/coordination.rs"]
mod coordination;
#[cfg(test)]
mod adapter_cases;
