use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rumqttc::QoS;

use crate::connection::connect;
use crate::model::COMPONENT;
use crate::publish::now_ms;

fn state_root() -> String {
    harmonia_config_store::get_config(COMPONENT, "global", "state-root")
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("harmonia")
                .to_string_lossy()
                .to_string()
        })
}

fn offline_queue_path() -> PathBuf {
    PathBuf::from(state_root()).join("mqtt-offline-queue.db")
}

fn open_offline_queue_db() -> Result<Connection, String> {
    let path = offline_queue_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create offline queue dir failed: {e}"))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("open offline queue db failed: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS offline_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            device_id TEXT NOT NULL,
            topic TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_offline_messages_device_id_id
            ON offline_messages(device_id, id);",
    )
    .map_err(|e| format!("initialize offline queue db failed: {e}"))?;
    Ok(conn)
}

pub(crate) fn load_offline_queue() {
    let _ = open_offline_queue_db();
}

pub(crate) fn enqueue_offline_message(
    device_id: &str,
    topic: &str,
    payload: &str,
) -> Result<(), String> {
    let conn = open_offline_queue_db()?;
    conn.execute(
        "INSERT INTO offline_messages (device_id, topic, payload, created_at_ms)
         VALUES (?1, ?2, ?3, ?4)",
        params![device_id, topic, payload, now_ms() as i64],
    )
    .map_err(|e| format!("enqueue offline message failed: {e}"))?;
    Ok(())
}

pub(crate) fn take_offline_messages(device_id: &str) -> Result<Vec<(String, String)>, String> {
    let mut conn = open_offline_queue_db()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("open offline queue transaction failed: {e}"))?;
    let mut stmt = tx
        .prepare(
            "SELECT topic, payload
             FROM offline_messages
             WHERE device_id = ?1
             ORDER BY id ASC",
        )
        .map_err(|e| format!("prepare offline queue select failed: {e}"))?;
    let messages = stmt
        .query_map(params![device_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query offline queue failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read offline queue rows failed: {e}"))?;
    drop(stmt);
    tx.execute(
        "DELETE FROM offline_messages WHERE device_id = ?1",
        params![device_id],
    )
    .map_err(|e| format!("delete offline queue rows failed: {e}"))?;
    tx.commit()
        .map_err(|e| format!("commit offline queue transaction failed: {e}"))?;
    Ok(messages)
}

/// Flush offline-queued messages for a device via MQTT publish.
pub(crate) fn flush_offline_queue(device_id: &str) {
    let messages = match take_offline_messages(device_id) {
        Ok(messages) => messages,
        Err(_) => return,
    };

    if messages.is_empty() {
        return;
    }

    let (client, mut connection) = match connect("flush") {
        Ok(v) => v,
        Err(_) => return,
    };
    for (topic, payload) in &messages {
        let _ = client.publish(topic.clone(), QoS::AtLeastOnce, false, payload.as_bytes());
    }
    // Drain connection events briefly to ensure delivery
    let deadline = Instant::now() + Duration::from_millis(2000);
    for event in connection.iter() {
        match event {
            Ok(_) => {}
            Err(_) => break,
        }
        if Instant::now() > deadline {
            break;
        }
    }
}
