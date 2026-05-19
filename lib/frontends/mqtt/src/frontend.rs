//! [`MqttFrontend`] — actor-owned MQTT 5 frontend.
//!
//! The frontend is "dumb transport": it terminates the MQTT 5 protocol,
//! decodes inbound publishes into [`InboundMessage`], and publishes
//! outbound payloads as-is. PGP signature verification of inbound
//! payloads happens at the **gateway**, not here, so MQTT/HTTP/Email
//! all share one verification path and policy reads uniform
//! `:auth-method` / `:auth-level` / `:auth-fp` metadata.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rumqttc::v5::mqttbytes::v5::Publish;
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::{AsyncClient, Event, EventLoop, Incoming, MqttOptions};
use rumqttc::Transport;

use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

use crate::tls::{build_rustls_client_config, load_config, MqttFrontendConfig};

pub struct MqttFrontend {
    client: AsyncClient,
    inbound: Arc<Mutex<VecDeque<InboundMessage>>>,
    agent_fp: String,
    /// Background task draining the rumqttc event loop. Held to keep the
    /// connection alive for the lifetime of the actor.
    _connection_task: tokio::task::JoinHandle<()>,
}

impl MqttFrontend {
    fn build_options(config: &MqttFrontendConfig) -> Result<MqttOptions, String> {
        let client_id = format!("agent-{}", config.agent_fp);
        let mut opts = MqttOptions::new(client_id, &config.broker_host, config.broker_port);
        opts.set_keep_alive(Duration::from_secs(config.keep_alive_secs));
        opts.set_clean_start(true);
        if let Some(tls) = &config.tls {
            let rustls_config = build_rustls_client_config(tls)?;
            opts.set_transport(Transport::tls_with_config(rumqttc::TlsConfiguration::Rustls(
                rustls_config,
            )));
        }
        Ok(opts)
    }

    fn spawn_event_loop(
        mut event_loop: EventLoop,
        inbound: Arc<Mutex<VecDeque<InboundMessage>>>,
        agent_fp: String,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match event_loop.poll().await {
                    Ok(Event::Incoming(Incoming::Publish(publish))) => {
                        if let Some(msg) = decode_inbound(&publish, &agent_fp) {
                            if let Ok(mut q) = inbound.lock() {
                                q.push_back(msg);
                            }
                        }
                    }
                    Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                        eprintln!("[INFO] [mqtt] connected to broker");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("[WARN] [mqtt] event loop error: {e}; reconnecting in 2s");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        })
    }
}

fn decode_inbound(publish: &Publish, agent_fp: &str) -> Option<InboundMessage> {
    let topic = std::str::from_utf8(&publish.topic).ok()?.to_string();
    let raw = std::str::from_utf8(&publish.payload).ok()?.to_string();

    // Parse the client_fp out of the topic so the gateway envelope addresses
    // it correctly. Topics look like `harmonia/{agent_fp}/inbox/{client_fp}`.
    let parts: Vec<&str> = topic.split('/').collect();
    let client_fp = if parts.len() >= 4 && parts[0] == "harmonia" && parts[1] == agent_fp {
        parts[3]
    } else {
        // System topic or unrecognised — use the whole topic as the address.
        topic.as_str()
    };

    let metadata = format!(
        "(:channel-class \"mqtt\" :node-id \"{}\" :remote t :transport-security \"mtls\" :topic \"{}\")",
        client_fp.replace('\\', "\\\\").replace('"', "\\\""),
        topic.replace('\\', "\\\\").replace('"', "\\\"")
    );
    Some(InboundMessage {
        address: client_fp.to_string(),
        text: raw,
        metadata: Some(metadata),
    })
}

#[async_trait]
impl Frontend for MqttFrontend {
    type Config = ();

    fn name() -> &'static str {
        "mqtt"
    }

    fn security_label() -> &'static str {
        // mTLS at the broker establishes the device-paired transport.
        // The gateway's PGP-verify pass stamps `:auth-level` on top.
        "authenticated"
    }

    async fn init(_: ()) -> Result<Self, String> {
        let config = load_config()?;
        let options = Self::build_options(&config)?;
        let (client, event_loop) = AsyncClient::new(options, 64);
        let inbound: Arc<Mutex<VecDeque<InboundMessage>>> = Arc::new(Mutex::new(VecDeque::new()));

        for topic in &config.subscribe_topics {
            client
                .subscribe(topic, QoS::AtLeastOnce)
                .await
                .map_err(|e| format!("mqtt subscribe {topic} failed: {e}"))?;
        }

        let connection_task =
            Self::spawn_event_loop(event_loop, inbound.clone(), config.agent_fp.clone());

        eprintln!(
            "[INFO] [mqtt] frontend ready (broker={}:{}, agent_fp={})",
            config.broker_host, config.broker_port, config.agent_fp
        );

        Ok(Self {
            client,
            inbound,
            agent_fp: config.agent_fp,
            _connection_task: connection_task,
        })
    }

    async fn poll(&mut self) -> PollResult {
        let mut out = Vec::new();
        if let Ok(mut q) = self.inbound.lock() {
            while let Some(msg) = q.pop_front() {
                out.push(msg);
            }
        }
        Ok(out)
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        // `channel` is normally a destination client fingerprint, mapped to
        // `harmonia/{agent_fp}/outbox/{client_fp}`. Two reserved synthetic
        // channels carry agent-wide events:
        //   `__heartbeat__` → `harmonia/{agent_fp}/heartbeat`  (QoS 0)
        //   `__system__`    → `harmonia/{agent_fp}/system`     (QoS 1)
        let (topic, qos) = match channel {
            "__heartbeat__" => (
                format!("harmonia/{}/heartbeat", self.agent_fp),
                QoS::AtMostOnce,
            ),
            "__system__" => (
                format!("harmonia/{}/system", self.agent_fp),
                QoS::AtLeastOnce,
            ),
            client_fp => (
                format!("harmonia/{}/outbox/{}", self.agent_fp, client_fp),
                QoS::AtLeastOnce,
            ),
        };
        self.client
            .publish(&topic, qos, false, text.as_bytes().to_vec())
            .await
            .map_err(|e| format!("mqtt publish {topic} failed: {e}"))
    }

    async fn shutdown(&mut self) {
        let _ = self.client.disconnect().await;
        if let Ok(mut q) = self.inbound.lock() {
            q.clear();
        }
    }
}
