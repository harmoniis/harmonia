//! Handle/Service pattern — the universal dispatch trait.
//!
//! Free Monad architecture: commands are pure data, handlers are pure functions,
//! state transitions are explicit deltas. Mutation is confined to `apply()`.

use crate::MemoryError;

/// The universal service trait. Every component implements this.
///
/// - `Cmd`: command enum (Free Monad — describes what to do, no side effects)
/// - `Ok`: result enum (what each command produces)
/// - `Delta`: state transition (explicit, inspectable, composable)
///
/// The handler is PURE: takes immutable &self, returns (Delta, Ok).
/// Mutation happens ONLY in apply(). This is the single point of state change.
pub trait Service {
    type Cmd;
    type Ok;
    type Delta;

    /// Pure handler: immutable state -> (delta, result). No mutation.
    fn handle(&self, cmd: Self::Cmd) -> Result<(Self::Delta, Self::Ok), MemoryError>;

    /// Apply delta — the ONE mutation point. Called by the actor after handle().
    fn apply(&mut self, delta: Self::Delta);
}
