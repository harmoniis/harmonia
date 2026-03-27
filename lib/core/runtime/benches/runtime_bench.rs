//! Comprehensive benchmarks for the Harmonia actor runtime.
//!
//! Run: cargo bench -p harmonia-runtime
//!
//! Benchmark groups:
//!   1. registry    — lock-free slot-indexed component lookup
//!   2. ipc_parse   — sexp parsing and dispatch routing
//!   3. token       — nonce generation and constant-time verification
//!   4. bridge      — bounded queue enqueue/drain throughput
//!   5. actor       — ractor actor spawn, cast, call round-trip
//!   6. concurrent  — parallel IPC dispatch under contention
//!   7. e2e         — full end-to-end: spawn actors → dispatch → reply

use std::collections::VecDeque;
use std::hint::black_box;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

// ═══════════════════════════════════════════════════════════════════════
// 1. COMPONENT REGISTRY — lock-free slot-indexed lookup
// ═══════════════════════════════════════════════════════════════════════

mod registry_bench {
    use super::*;
    use arc_swap::ArcSwap;

    // Reproduce the slot-indexed registry inline (avoid private module import)
    const NUM_SLOTS: usize = 10;

    #[repr(u8)]
    #[derive(Clone, Copy)]
    enum Slot {
        Chronicle = 0, Gateway = 1, Tailnet = 2, Signalograd = 3,
        MemoryField = 4, Vault = 5, Config = 6, ProviderRouter = 7,
        Parallel = 8, Router = 9,
    }

    fn slot_from_name(name: &str) -> Option<Slot> {
        match name {
            "chronicle" => Some(Slot::Chronicle),
            "gateway" => Some(Slot::Gateway),
            "tailnet" => Some(Slot::Tailnet),
            "signalograd" => Some(Slot::Signalograd),
            "memory-field" => Some(Slot::MemoryField),
            "vault" => Some(Slot::Vault),
            "config" => Some(Slot::Config),
            "provider-router" => Some(Slot::ProviderRouter),
            "parallel" => Some(Slot::Parallel),
            "router" => Some(Slot::Router),
            _ => None,
        }
    }

    #[derive(Clone)]
    struct Inner { slots: [Option<u64>; NUM_SLOTS] }

    pub fn bench_registry(c: &mut Criterion) {
        let mut group = c.benchmark_group("registry");

        // Populate a registry
        let inner = Inner { slots: [Some(42); NUM_SLOTS] };
        let registry = Arc::new(ArcSwap::from_pointee(inner));

        let names = [
            "chronicle", "gateway", "tailnet", "signalograd",
            "memory-field", "vault", "config", "provider-router",
            "parallel", "router",
        ];

        // Single lookup
        group.bench_function("single_lookup", |b| {
            b.iter(|| {
                let slot = slot_from_name(black_box("vault")).unwrap();
                let guard = registry.load();
                black_box(guard.slots[slot as usize]);
            });
        });

        // All 10 components sequentially
        group.bench_function("all_10_lookups", |b| {
            b.iter(|| {
                for name in &names {
                    let slot = slot_from_name(black_box(name)).unwrap();
                    let guard = registry.load();
                    black_box(guard.slots[slot as usize]);
                }
            });
        });

        // Compare with HashMap baseline
        let hashmap: std::collections::HashMap<String, u64> = names
            .iter()
            .map(|n| (n.to_string(), 42u64))
            .collect();
        let hm_arc = Arc::new(std::sync::RwLock::new(hashmap));

        group.bench_function("hashmap_baseline_single", |b| {
            b.iter(|| {
                let guard = hm_arc.read().unwrap();
                black_box(guard.get(black_box("vault")));
            });
        });

        group.bench_function("hashmap_baseline_all_10", |b| {
            b.iter(|| {
                let guard = hm_arc.read().unwrap();
                for name in &names {
                    black_box(guard.get(black_box(*name)));
                }
            });
        });

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. IPC SEXP PARSING — extract values, dispatch routing
// ═══════════════════════════════════════════════════════════════════════

mod ipc_parse_bench {
    use super::*;

    fn extract_string_value(sexp: &str, key: &str) -> Option<String> {
        let idx = sexp.find(key)?;
        let after = &sexp[idx + key.len()..];
        let after = after.trim_start();
        if after.starts_with('"') {
            let inner = &after[1..];
            let bytes = inner.as_bytes();
            let mut end = 0;
            while end < bytes.len() {
                if bytes[end] == b'"' {
                    return Some(inner[..end].replace("\\\"", "\"").replace("\\\\", "\\"));
                }
                if bytes[end] == b'\\' { end += 1; }
                end += 1;
            }
            None
        } else {
            let val: String = after.chars().take_while(|c| !c.is_whitespace() && *c != ')').collect();
            if val.is_empty() { None } else { Some(val) }
        }
    }

    fn extract_u64_value(sexp: &str, key: &str) -> Option<u64> {
        let idx = sexp.find(key)?;
        let after = &sexp[idx + key.len()..];
        let after = after.trim_start();
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        num_str.parse().ok()
    }

    fn dispatch_route(sexp: &str) -> &'static str {
        let trimmed = sexp.trim();
        if trimmed.starts_with("(:drain") { "drain" }
        else if trimmed.starts_with("(:register") { "register" }
        else if trimmed.starts_with("(:deregister") { "deregister" }
        else if trimmed.starts_with("(:heartbeat") { "heartbeat" }
        else if trimmed.starts_with("(:post") { "post" }
        else if trimmed.starts_with("(:state") { "state" }
        else if trimmed.starts_with("(:list") { "list" }
        else if trimmed.starts_with("(:component") { "component" }
        else if trimmed.starts_with("(:modules") { "modules" }
        else if trimmed.starts_with("(:shutdown") { "shutdown" }
        else { "unknown" }
    }

    pub fn bench_ipc_parse(c: &mut Criterion) {
        let mut group = c.benchmark_group("ipc_parse");

        let component_sexp = r#"(:component "vault" :op "set-secret" :symbol "api-key" :value "sk-live-1234567890abcdef")"#;
        let heartbeat_sexp = "(:heartbeat :id 42 :bytes-delta 1024)";
        let register_sexp = r#"(:register :kind "cli-agent")"#;
        let large_sexp = format!(
            r#"(:component "chronicle" :op "query" :sql "{}")"#,
            "SELECT * FROM events WHERE ts > 0 ".repeat(100)
        );

        // Dispatch routing (starts_with chain)
        group.bench_function("route_component", |b| {
            b.iter(|| black_box(dispatch_route(black_box(component_sexp))));
        });
        group.bench_function("route_heartbeat", |b| {
            b.iter(|| black_box(dispatch_route(black_box(heartbeat_sexp))));
        });
        group.bench_function("route_shutdown_worst_case", |b| {
            b.iter(|| black_box(dispatch_route(black_box("(:shutdown)"))));
        });

        // String extraction
        group.bench_function("extract_component_name", |b| {
            b.iter(|| black_box(extract_string_value(black_box(component_sexp), ":component")));
        });
        group.bench_function("extract_op", |b| {
            b.iter(|| black_box(extract_string_value(black_box(component_sexp), ":op")));
        });
        group.bench_function("extract_u64", |b| {
            b.iter(|| black_box(extract_u64_value(black_box(heartbeat_sexp), ":id")));
        });

        // Full parse of a component dispatch (all fields)
        group.bench_function("full_component_parse", |b| {
            b.iter(|| {
                let s = black_box(component_sexp);
                let _route = dispatch_route(s);
                let _comp = extract_string_value(s, ":component");
                let _op = extract_string_value(s, ":op");
                let _sym = extract_string_value(s, ":symbol");
                let _val = extract_string_value(s, ":value");
            });
        });

        // Large payload
        group.bench_with_input(
            BenchmarkId::new("large_payload_parse", "4KB"),
            &large_sexp,
            |b, sexp| {
                b.iter(|| {
                    let _route = dispatch_route(black_box(sexp));
                    let _comp = extract_string_value(black_box(sexp), ":component");
                    let _sql = extract_string_value(black_box(sexp), ":sql");
                });
            },
        );

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 3. TOKEN — nonce generation and constant-time verification
// ═══════════════════════════════════════════════════════════════════════

mod token_bench {
    use super::*;
    use rand::Rng;

    fn generate_token() -> String {
        let bytes: [u8; 32] = rand::thread_rng().gen();
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() { return false; }
        let mut diff = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }

    fn fnv1a_64(data: &[u8]) -> u64 {
        let mut hash: u64 = 0xCBF29CE484222325;
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001B3);
        }
        hash
    }

    pub fn bench_token(c: &mut Criterion) {
        let mut group = c.benchmark_group("token");

        group.bench_function("generate_32byte_token", |b| {
            b.iter(|| black_box(generate_token()));
        });

        let token_a = generate_token();
        let token_b = token_a.clone();
        let token_c = generate_token(); // different

        group.bench_function("verify_matching_token", |b| {
            b.iter(|| black_box(constant_time_eq(
                black_box(token_a.as_bytes()),
                black_box(token_b.as_bytes()),
            )));
        });

        group.bench_function("verify_mismatched_token", |b| {
            b.iter(|| black_box(constant_time_eq(
                black_box(token_a.as_bytes()),
                black_box(token_c.as_bytes()),
            )));
        });

        // FNV-1a hashing (IPC name derivation)
        group.bench_function("fnv1a_short_path", |b| {
            b.iter(|| black_box(fnv1a_64(black_box(b"/tmp/harmonia"))));
        });
        group.bench_function("fnv1a_long_path", |b| {
            b.iter(|| black_box(fnv1a_64(
                black_box(b"/home/george/.harmoniis/harmonia/state"),
            )));
        });

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 4. BRIDGE — bounded queue enqueue/drain throughput
// ═══════════════════════════════════════════════════════════════════════

mod bridge_bench {
    use super::*;
    use harmonia_actor_protocol::{ActorKind, HarmoniaMessage, MessagePayload};

    fn make_msg(id: u64) -> HarmoniaMessage {
        HarmoniaMessage {
            id,
            source: 1,
            target: 0,
            kind: ActorKind::Gateway,
            timestamp: 1700000000,
            payload: MessagePayload::ProgressHeartbeat { bytes_delta: 1024 },
        }
    }

    pub fn bench_bridge(c: &mut Criterion) {
        let mut group = c.benchmark_group("bridge");

        // Enqueue throughput
        group.bench_function("enqueue_1", |b| {
            let mut queue: VecDeque<HarmoniaMessage> = VecDeque::with_capacity(256);
            b.iter(|| {
                queue.push_back(black_box(make_msg(1)));
                if queue.len() > 4096 { queue.pop_front(); }
            });
            // Reset
            queue.clear();
        });

        // Drain throughput at various fill levels
        for count in [1, 10, 100, 1000] {
            group.throughput(Throughput::Elements(count as u64));
            group.bench_with_input(
                BenchmarkId::new("drain_to_sexp", count),
                &count,
                |b, &count| {
                    let mut queue: VecDeque<HarmoniaMessage> = (0..count)
                        .map(|i| make_msg(i as u64))
                        .collect();
                    let mut buf = String::with_capacity(4096);
                    b.iter(|| {
                        // Refill queue for each iteration
                        if queue.is_empty() {
                            for i in 0..count {
                                queue.push_back(make_msg(i as u64));
                            }
                        }
                        buf.clear();
                        buf.push('(');
                        for (i, msg) in queue.drain(..).enumerate() {
                            if i > 0 { buf.push(' '); }
                            msg.write_sexp(&mut buf);
                        }
                        buf.push(')');
                        black_box(&buf);
                    });
                },
            );
        }

        // write_sexp vs to_sexp allocation comparison
        let msg = make_msg(42);
        group.bench_function("to_sexp_allocating", |b| {
            b.iter(|| black_box(msg.to_sexp()));
        });
        group.bench_function("write_sexp_zero_alloc", |b| {
            let mut buf = String::with_capacity(512);
            b.iter(|| {
                buf.clear();
                msg.write_sexp(&mut buf);
                black_box(&buf);
            });
        });

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 5. ACTOR — ractor actor spawn, cast, call round-trip
// ═══════════════════════════════════════════════════════════════════════

mod actor_bench {
    use super::*;

    // Minimal echo actor for benchmarking pure ractor overhead
    struct EchoActor;

    #[derive(Debug)]
    enum EchoMsg {
        Ping,
        Echo(String, RpcReplyPort<String>),
    }

    impl Actor for EchoActor {
        type Msg = EchoMsg;
        type State = u64; // message counter
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: (),
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(0)
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            *state += 1;
            match message {
                EchoMsg::Ping => {}
                EchoMsg::Echo(s, reply) => { let _ = reply.send(s); }
            }
            Ok(())
        }
    }

    pub fn bench_actors(c: &mut Criterion) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut group = c.benchmark_group("actor");

        // Actor spawn cost
        group.bench_function("spawn_and_stop", |b| {
            b.to_async(&rt).iter(|| async {
                let (actor, handle) = Actor::spawn(None, EchoActor, ())
                    .await.unwrap();
                actor.stop(None);
                let _ = handle.await;
            });
        });

        // Fire-and-forget cast throughput
        group.bench_function("cast_ping", |b| {
            let actor = rt.block_on(async {
                Actor::spawn(None, EchoActor, ()).await.unwrap().0
            });
            b.iter(|| {
                let _ = actor.cast(EchoMsg::Ping);
            });
            actor.stop(None);
        });

        // RPC call round-trip (send + wait for reply)
        group.bench_function("call_echo_roundtrip", |b| {
            let actor = rt.block_on(async {
                Actor::spawn(None, EchoActor, ()).await.unwrap().0
            });
            b.to_async(&rt).iter(|| {
                let a = actor.clone();
                async move {
                    let result = ractor::call_t!(a, EchoMsg::Echo, 5000, "ping".to_string());
                    black_box(result.unwrap());
                }
            });
            actor.stop(None);
        });

        // Burst: 1000 casts then 1 call (drain confirmation)
        group.bench_function("burst_1000_cast_then_call", |b| {
            let actor = rt.block_on(async {
                Actor::spawn(None, EchoActor, ()).await.unwrap().0
            });
            b.to_async(&rt).iter(|| {
                let a = actor.clone();
                async move {
                    for _ in 0..1000 {
                        let _ = a.cast(EchoMsg::Ping);
                    }
                    let result = ractor::call_t!(a, EchoMsg::Echo, 10000, "done".to_string());
                    black_box(result.unwrap());
                }
            });
            actor.stop(None);
        });

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. CONCURRENT — parallel dispatch under contention
// ═══════════════════════════════════════════════════════════════════════

mod concurrent_bench {
    use super::*;
    use arc_swap::ArcSwap;

    pub fn bench_concurrent(c: &mut Criterion) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut group = c.benchmark_group("concurrent");
        group.sample_size(50);

        // Concurrent registry reads (simulates IPC handler contention)
        let inner = [Some(42u64); 10];
        let registry = Arc::new(ArcSwap::from_pointee(inner));

        for n_tasks in [1, 4, 8, 16, 32] {
            group.bench_with_input(
                BenchmarkId::new("registry_read_contention", n_tasks),
                &n_tasks,
                |b, &n| {
                    b.to_async(&rt).iter(|| {
                        let reg = registry.clone();
                        async move {
                            let mut handles = Vec::with_capacity(n);
                            for _ in 0..n {
                                let r = reg.clone();
                                handles.push(tokio::spawn(async move {
                                    for i in 0..1000 {
                                        let guard = r.load();
                                        black_box(guard[i % 10]);
                                    }
                                }));
                            }
                            for h in handles {
                                h.await.unwrap();
                            }
                        }
                    });
                },
            );
        }

        // Concurrent actor calls
        struct CounterActor;
        #[derive(Debug)]
        enum CounterMsg {
            Inc,
            Get(RpcReplyPort<u64>),
        }
        impl Actor for CounterActor {
            type Msg = CounterMsg;
            type State = u64;
            type Arguments = ();
            async fn pre_start(&self, _: ActorRef<Self::Msg>, _: ()) -> Result<u64, ActorProcessingErr> { Ok(0) }
            async fn handle(&self, _: ActorRef<Self::Msg>, msg: Self::Msg, state: &mut u64) -> Result<(), ActorProcessingErr> {
                match msg {
                    CounterMsg::Inc => *state += 1,
                    CounterMsg::Get(r) => { let _ = r.send(*state); }
                }
                Ok(())
            }
        }

        for n_tasks in [1, 4, 8, 16] {
            group.bench_with_input(
                BenchmarkId::new("actor_call_contention", n_tasks),
                &n_tasks,
                |b, &n| {
                    let actor = rt.block_on(async {
                        Actor::spawn(None, CounterActor, ()).await.unwrap().0
                    });
                    b.to_async(&rt).iter(|| {
                        let a = actor.clone();
                        async move {
                            let mut handles = Vec::with_capacity(n);
                            for _ in 0..n {
                                let aa = a.clone();
                                handles.push(tokio::spawn(async move {
                                    for _ in 0..100 {
                                        let _ = aa.cast(CounterMsg::Inc);
                                    }
                                    let v = ractor::call_t!(aa, CounterMsg::Get, 5000);
                                    black_box(v.unwrap());
                                }));
                            }
                            for h in handles {
                                h.await.unwrap();
                            }
                        }
                    });
                    actor.stop(None);
                },
            );
        }

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. SEXP SERIALIZATION — message encoding overhead
// ═══════════════════════════════════════════════════════════════════════

mod sexp_bench {
    use super::*;
    use harmonia_actor_protocol::*;

    pub fn bench_sexp(c: &mut Criterion) {
        let mut group = c.benchmark_group("sexp");

        let simple_msg = HarmoniaMessage {
            id: 1, source: 5, target: 0,
            kind: ActorKind::Gateway,
            timestamp: 1700000000,
            payload: MessagePayload::ProgressHeartbeat { bytes_delta: 1024 },
        };

        let complex_msg = HarmoniaMessage {
            id: 2, source: 3, target: 0,
            kind: ActorKind::Signalograd,
            timestamp: 1700000001,
            payload: MessagePayload::SupervisionVerdict {
                task: 10, spec: 5, passed: 8, failed: 2, skipped: 0,
                confidence: 0.95,
                grade: "A".to_string(),
                summary: "All critical assertions passed with high confidence".to_string(),
            },
        };

        let escaped_msg = HarmoniaMessage {
            id: 3, source: 1, target: 0,
            kind: ActorKind::Tool,
            timestamp: 1700000002,
            payload: MessagePayload::ToolCompleted {
                tool_name: "browser".to_string(),
                operation: "navigate".to_string(),
                request_id: 42,
                envelope_sexp: r#"(:result "page loaded with \"quotes\" and \\backslashes")"#.to_string(),
                duration_ms: 1500,
            },
        };

        group.bench_function("simple_to_sexp", |b| {
            b.iter(|| black_box(simple_msg.to_sexp()));
        });
        group.bench_function("simple_write_sexp", |b| {
            let mut buf = String::with_capacity(256);
            b.iter(|| { buf.clear(); simple_msg.write_sexp(&mut buf); black_box(&buf); });
        });
        group.bench_function("complex_to_sexp", |b| {
            b.iter(|| black_box(complex_msg.to_sexp()));
        });
        group.bench_function("complex_write_sexp", |b| {
            let mut buf = String::with_capacity(512);
            b.iter(|| { buf.clear(); complex_msg.write_sexp(&mut buf); black_box(&buf); });
        });
        group.bench_function("escaped_to_sexp", |b| {
            b.iter(|| black_box(escaped_msg.to_sexp()));
        });
        group.bench_function("escaped_write_sexp", |b| {
            let mut buf = String::with_capacity(512);
            b.iter(|| { buf.clear(); escaped_msg.write_sexp(&mut buf); black_box(&buf); });
        });

        // Batch serialization (simulates bridge drain)
        let batch: Vec<HarmoniaMessage> = (0..100).map(|i| HarmoniaMessage {
            id: i, source: 1, target: 0,
            kind: ActorKind::Gateway,
            timestamp: 1700000000 + i,
            payload: MessagePayload::ProgressHeartbeat { bytes_delta: i * 10 },
        }).collect();

        group.throughput(Throughput::Elements(100));
        group.bench_function("batch_100_write_sexp", |b| {
            let mut buf = String::with_capacity(16384);
            b.iter(|| {
                buf.clear();
                buf.push('(');
                for (i, msg) in batch.iter().enumerate() {
                    if i > 0 { buf.push(' '); }
                    msg.write_sexp(&mut buf);
                }
                buf.push(')');
                black_box(&buf);
            });
        });

        group.finish();
    }
}

// ═══════════════════════════════════════════════════════════════════════
// CRITERION MAIN
// ═══════════════════════════════════════════════════════════════════════

criterion_group!(
    benches,
    registry_bench::bench_registry,
    ipc_parse_bench::bench_ipc_parse,
    token_bench::bench_token,
    bridge_bench::bench_bridge,
    actor_bench::bench_actors,
    concurrent_bench::bench_concurrent,
    sexp_bench::bench_sexp,
);
criterion_main!(benches);
