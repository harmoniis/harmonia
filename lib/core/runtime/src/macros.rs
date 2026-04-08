//! Declarative macros for Harmonia component metaprogramming.
//!
//! One macro invocation = one fully wired actor.
//! No boilerplate. Pure functional. Declarative.

/// Declare a stateless component actor that dispatches to `crate::dispatch::dispatch(name, sexp)`.
///
/// Two variants:
///   - `declare_stateless_component!(name, StructName, "dispatch-name")` — synchronous dispatch
///   - `declare_stateless_component!(name, StructName, "dispatch-name", blocking)` — spawn_blocking for I/O
///
/// Generates: pub struct, Actor impl with Tick/Dispatch/Shutdown handling.
macro_rules! declare_stateless_component {
    // Synchronous dispatch variant.
    ($struct_name:ident, $dispatch_name:literal) => {
        pub struct $struct_name;

        impl ::ractor::Actor for $struct_name {
            type Msg = $crate::actors::ComponentMsg;
            type State = ();
            type Arguments = ();

            async fn pre_start(
                &self,
                _myself: ::ractor::ActorRef<Self::Msg>,
                _args: (),
            ) -> Result<Self::State, ::ractor::ActorProcessingErr> {
                eprintln!(concat!("[INFO] [runtime] ", stringify!($struct_name), " started"));
                Ok(())
            }

            async fn handle(
                &self,
                _myself: ::ractor::ActorRef<Self::Msg>,
                message: Self::Msg,
                _state: &mut Self::State,
            ) -> Result<(), ::ractor::ActorProcessingErr> {
                match message {
                    $crate::actors::ComponentMsg::Dispatch(sexp, reply) => {
                        let result = $crate::dispatch::dispatch($dispatch_name, &sexp);
                        let _ = reply.send(result);
                    }
                    $crate::actors::ComponentMsg::Shutdown => {
                        eprintln!(concat!("[INFO] [runtime] ", stringify!($struct_name), " shutting down"));
                    }
                    $crate::actors::ComponentMsg::Tick => {}
                }
                Ok(())
            }
        }
    };

    // Blocking dispatch variant (for file I/O, shell exec, network calls).
    ($struct_name:ident, $dispatch_name:literal, blocking) => {
        pub struct $struct_name;

        impl ::ractor::Actor for $struct_name {
            type Msg = $crate::actors::ComponentMsg;
            type State = ();
            type Arguments = ();

            async fn pre_start(
                &self,
                _myself: ::ractor::ActorRef<Self::Msg>,
                _args: (),
            ) -> Result<Self::State, ::ractor::ActorProcessingErr> {
                eprintln!(concat!("[INFO] [runtime] ", stringify!($struct_name), " started"));
                Ok(())
            }

            async fn handle(
                &self,
                _myself: ::ractor::ActorRef<Self::Msg>,
                message: Self::Msg,
                _state: &mut Self::State,
            ) -> Result<(), ::ractor::ActorProcessingErr> {
                match message {
                    $crate::actors::ComponentMsg::Dispatch(sexp, reply) => {
                        let result = ::tokio::task::spawn_blocking(move || {
                            $crate::dispatch::dispatch($dispatch_name, &sexp)
                        })
                        .await
                        .unwrap_or_else(|e| format!("(:error \"{} join: {}\")", $dispatch_name, e));
                        let _ = reply.send(result);
                    }
                    $crate::actors::ComponentMsg::Shutdown => {
                        eprintln!(concat!("[INFO] [runtime] ", stringify!($struct_name), " shutting down"));
                    }
                    $crate::actors::ComponentMsg::Tick => {}
                }
                Ok(())
            }
        }
    };
}

/// Declare a component actor from a ComponentDescriptor trait impl.
///
/// This is the NEW way to define components. ONE macro = complete actor.
/// The ComponentDescriptor trait defines init/dispatch/tick/shutdown.
/// This macro generates the ractor Actor impl that wires it all together.
///
/// ```ignore
/// declare_component!(MemPalaceComponent, MemPalaceActor, MemPalaceActorState);
/// ```
macro_rules! declare_component {
    ($descriptor:ty, $actor_name:ident, $state_name:ident) => {
        pub struct $actor_name;
        pub struct $state_name {
            pub(crate) inner: <$descriptor as ::harmonia_actor_protocol::ComponentDescriptor>::State,
        }
        impl ::ractor::Actor for $actor_name {
            type Msg = $crate::actors::ComponentMsg;
            type State = $state_name;
            type Arguments = ();
            async fn pre_start(
                &self, _myself: ::ractor::ActorRef<Self::Msg>, _args: (),
            ) -> Result<Self::State, ::ractor::ActorProcessingErr> {
                let inner = <$descriptor as ::harmonia_actor_protocol::ComponentDescriptor>::init();
                eprintln!("[INFO] [runtime] {} started",
                    <$descriptor as ::harmonia_actor_protocol::ComponentDescriptor>::NAME);
                Ok($state_name { inner })
            }
            async fn handle(
                &self, _myself: ::ractor::ActorRef<Self::Msg>,
                message: Self::Msg, state: &mut Self::State,
            ) -> Result<(), ::ractor::ActorProcessingErr> {
                match message {
                    $crate::actors::ComponentMsg::Tick => {
                        <$descriptor as ::harmonia_actor_protocol::ComponentDescriptor>::tick(&mut state.inner);
                    }
                    $crate::actors::ComponentMsg::Dispatch(sexp, reply) => {
                        let result = <$descriptor as ::harmonia_actor_protocol::ComponentDescriptor>::dispatch(&mut state.inner, &sexp);
                        let _ = reply.send(result);
                    }
                    $crate::actors::ComponentMsg::Shutdown => {
                        <$descriptor as ::harmonia_actor_protocol::ComponentDescriptor>::shutdown(&mut state.inner);
                        eprintln!("[INFO] [runtime] {} shutting down",
                            <$descriptor as ::harmonia_actor_protocol::ComponentDescriptor>::NAME);
                    }
                }
                Ok(())
            }
        }
    };
}

pub(crate) use declare_stateless_component;
pub(crate) use declare_component;
