//! `Frontend` — the shared interface every messaging frontend implements.
//!
//! Each concrete frontend (Slack, Discord, Telegram, WhatsApp, Signal, …)
//! provides a struct that owns its own state (no `OnceLock`/`RwLock`
//! singletons) and implements [`Frontend`]. The [`FrontendActor`] wrapper
//! turns any such struct into a `ractor` actor with a uniform message API,
//! so the gateway/runtime can fan polls and sends out to every frontend
//! through `ActorRef<FrontendMsg>` regardless of transport.

use async_trait::async_trait;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

/// One inbound message produced by a frontend's `poll`.
///
/// `address` is the channel/conversation identifier in the frontend's
/// vocabulary (Slack channel id, Telegram chat id, phone number, …).
/// `metadata` is an optional s-expression carrying frontend-specific
/// context (sender id, channel-class, remote-flag) that the gateway
/// embeds in the resulting envelope.
#[derive(Debug, Clone)]
pub struct InboundMessage {
    pub address: String,
    pub text: String,
    pub metadata: Option<String>,
}

/// Result of polling a frontend for new inbound messages.
pub type PollResult = Result<Vec<InboundMessage>, String>;

/// The message protocol every frontend actor speaks.
///
/// Variants are tuple-shaped so they compose with `ractor::call_t!` — the
/// last positional is always the `RpcReplyPort`, all earlier positionals are
/// the call arguments.
pub enum FrontendMsg {
    /// Poll for new inbound messages.
    Poll(RpcReplyPort<PollResult>),
    /// `Send(channel, text, reply)` — send `text` to `channel`.
    Send(String, String, RpcReplyPort<Result<(), String>>),
    /// Graceful shutdown — frontend frees external resources.
    Shutdown,
}

/// Trait implemented by each concrete frontend.
///
/// `init` constructs the frontend from a config value (typically an
/// s-expression string or a parsed config struct), `poll`/`send` are the
/// per-tick I/O, and `shutdown` is the optional teardown.
///
/// State lives on `Self`. There must be no module-level `OnceLock`,
/// `RwLock`, or `static mut` — the actor wrapper owns the only instance.
#[async_trait]
pub trait Frontend: Send + Sized + 'static {
    /// Frontend-specific config payload (e.g. an s-expression string).
    type Config: Send + 'static;

    /// Stable name used by the gateway dispatcher (`"slack"`, `"telegram"`).
    fn name() -> &'static str;

    /// Default security label envelopes from this frontend carry — gets
    /// stamped onto the gateway envelope so downstream policy can gate on it
    /// uniformly across frontends. Local/device-paired transports (TUI,
    /// MQTT-mTLS) override this to `"owner"`; remote messaging frontends
    /// keep the default `"authenticated"`.
    fn security_label() -> &'static str {
        "authenticated"
    }

    /// Build the frontend from its config.
    async fn init(config: Self::Config) -> Result<Self, String>;

    /// Poll for new inbound messages.
    async fn poll(&mut self) -> PollResult;

    /// Send a text message to a channel/conversation.
    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String>;

    /// Optional teardown — default is no-op.
    async fn shutdown(&mut self) {}
}

/// Generic actor wrapper that drives a [`Frontend`].
///
/// One instance per concrete frontend, spawned at runtime boot. The actor
/// owns the [`Frontend`] value as its state — pure-functional discipline
/// is preserved at the actor boundary even if individual `poll`/`send`
/// implementations mutate internal book-keeping (e.g. `last_ts`).
pub struct FrontendActor<F: Frontend>(std::marker::PhantomData<F>);

impl<F: Frontend> Default for FrontendActor<F> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<F> Actor for FrontendActor<F>
where
    F: Frontend + Sync,
    F::Config: Sync,
{
    type Msg = FrontendMsg;
    type State = F;
    type Arguments = F::Config;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        config: F::Config,
    ) -> Result<Self::State, ActorProcessingErr> {
        F::init(config).await.map_err(|e| {
            ActorProcessingErr::from(format!("frontend {} init failed: {e}", F::name()))
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            FrontendMsg::Poll(reply) => {
                let r = state.poll().await;
                let _ = reply.send(r);
            }
            FrontendMsg::Send(channel, text, reply) => {
                let r = state.send(&channel, &text).await;
                let _ = reply.send(r);
            }
            FrontendMsg::Shutdown => {
                state.shutdown().await;
            }
        }
        Ok(())
    }
}

/// Spawn a frontend actor under a parent supervisor.
///
/// The actor is registered with the supervisor as a linked child so a
/// parent crash cleans the actor up; restart handling is the supervisor's
/// responsibility (see `runtime::supervisor`).
pub async fn spawn_frontend<F, P>(
    supervisor: &ActorRef<P>,
    config: F::Config,
) -> Result<ActorRef<FrontendMsg>, String>
where
    F: Frontend + Sync,
    F::Config: Sync,
    P: ractor::Message,
{
    let (actor_ref, _handle) = Actor::spawn_linked(
        Some(format!("frontend-{}", F::name())),
        FrontendActor::<F>::default(),
        config,
        supervisor.get_cell(),
    )
    .await
    .map_err(|e| format!("spawn frontend {}: {e}", F::name()))?;
    Ok(actor_ref)
}
