use rusqlite::params;

use crate::db;

/// Store a complete concept graph snapshot: the raw s-expression AND its
/// decomposed nodes/edges for relational SQL traversal.
pub fn record_snapshot(
    source: &str,
    sexp: &str,
    nodes: &[(String, String, i32, i32, i32, String)], // (concept, domain, count, depth_min, depth_max, classes)
    edges: &[(String, String, i32, bool, String)],     // (a, b, weight, interdisciplinary, reasons)
) -> Result<i64, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;

    let inter_count = edges.iter().filter(|e| e.3).count() as i32;

    // Compute a simple digest for dedup
    let digest = format!("{:x}", md5_simple(sexp));

    lock.execute(
        "INSERT INTO graph_snapshots
            (source, node_count, edge_count, interdisciplinary_edges, sexp, digest)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            source,
            nodes.len() as i32,
            edges.len() as i32,
            inter_count,
            sexp,
            digest,
        ],
    )
    .map_err(|e| e.to_string())?;

    let snapshot_id = lock.last_insert_rowid();

    // Batch insert nodes
    {
        let mut stmt = lock
            .prepare(
                "INSERT INTO graph_nodes
                    (snapshot_id, concept, domain, count, depth_min, depth_max, classes)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
            )
            .map_err(|e| e.to_string())?;
        for (concept, domain, count, depth_min, depth_max, classes) in nodes {
            stmt.execute(params![
                snapshot_id,
                concept,
                domain,
                *count,
                *depth_min,
                *depth_max,
                classes,
            ])
            .map_err(|e| e.to_string())?;
        }
    }

    // Batch insert edges
    {
        let mut stmt = lock
            .prepare(
                "INSERT INTO graph_edges
                    (snapshot_id, node_a, node_b, weight, interdisciplinary, reasons)
                 VALUES (?1,?2,?3,?4,?5,?6)",
            )
            .map_err(|e| e.to_string())?;
        for (a, b, weight, inter, reasons) in edges {
            stmt.execute(params![snapshot_id, a, b, *weight, *inter as i32, reasons])
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(snapshot_id)
}

/// Simple non-cryptographic hash for dedup (not security-critical).
fn md5_simple(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Query: find all concepts connected to a given concept within N hops.
/// Uses recursive CTE for graph traversal — this is the power of SQL on graph data.
pub fn traverse_from(
    concept: &str,
    max_hops: i32,
    snapshot_id: Option<i64>,
) -> Result<String, String> {
    let snap_filter = match snapshot_id {
        Some(id) => format!("AND snapshot_id = {}", id),
        None => {
            // Use latest snapshot
            "AND snapshot_id = (SELECT MAX(id) FROM graph_snapshots)".to_string()
        }
    };

    let sql = format!(
        "WITH RECURSIVE reachable(concept, hop, path) AS (
            SELECT ?1, 0, ?1
            UNION ALL
            SELECT
                CASE WHEN e.node_a = r.concept THEN e.node_b ELSE e.node_a END,
                r.hop + 1,
                r.path || ' -> ' || CASE WHEN e.node_a = r.concept THEN e.node_b ELSE e.node_a END
            FROM reachable r
            JOIN graph_edges e ON (e.node_a = r.concept OR e.node_b = r.concept) {snap}
            WHERE r.hop < ?2
              AND INSTR(r.path, CASE WHEN e.node_a = r.concept THEN e.node_b ELSE e.node_a END) = 0
        )
        SELECT DISTINCT concept, MIN(hop) AS min_hop, path
        FROM reachable
        WHERE concept != ?1
        GROUP BY concept
        ORDER BY min_hop, concept
        LIMIT 100",
        snap = snap_filter
    );

    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock.prepare(&sql).map_err(|e| e.to_string())?;
    let mut rows_sexp = Vec::new();
    let mut rows = stmt
        .query(params![concept, max_hops])
        .map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let c: String = row.get(0).map_err(|e| e.to_string())?;
        let hop: i32 = row.get(1).map_err(|e| e.to_string())?;
        let path: String = row.get(2).map_err(|e| e.to_string())?;
        rows_sexp.push(format!(
            "(:concept \"{}\" :hops {} :path \"{}\")",
            c.replace('"', "\\\""),
            hop,
            path.replace('"', "\\\"")
        ));
    }
    Ok(format!("({})", rows_sexp.join("\n ")))
}

/// Query: find interdisciplinary bridges — edges connecting different knowledge domains.
pub fn interdisciplinary_bridges(snapshot_id: Option<i64>) -> Result<String, String> {
    let snap_filter = match snapshot_id {
        Some(id) => format!("WHERE e.snapshot_id = {}", id),
        None => "WHERE e.snapshot_id = (SELECT MAX(id) FROM graph_snapshots)".to_string(),
    };

    let sql = format!(
        "SELECT e.node_a, e.node_b, e.weight, e.reasons,
                na.domain AS domain_a, nb.domain AS domain_b
         FROM graph_edges e
         JOIN graph_nodes na ON na.snapshot_id = e.snapshot_id AND na.concept = e.node_a
         JOIN graph_nodes nb ON nb.snapshot_id = e.snapshot_id AND nb.concept = e.node_b
         {filter}
           AND e.interdisciplinary = 1
         ORDER BY e.weight DESC
         LIMIT 50",
        filter = snap_filter
    );

    db::query_sexp(&sql)
}

/// Query: domain distribution — how many concepts per domain in latest snapshot.
pub fn domain_distribution(snapshot_id: Option<i64>) -> Result<String, String> {
    let snap_filter = match snapshot_id {
        Some(id) => format!("WHERE snapshot_id = {}", id),
        None => "WHERE snapshot_id = (SELECT MAX(id) FROM graph_snapshots)".to_string(),
    };

    let sql = format!(
        "SELECT domain, COUNT(*) as node_count, SUM(count) as total_refs
         FROM graph_nodes
         {filter}
         GROUP BY domain
         ORDER BY total_refs DESC",
        filter = snap_filter
    );

    db::query_sexp(&sql)
}

/// Query: most connected concepts (highest degree centrality).
pub fn central_concepts(snapshot_id: Option<i64>, limit: i32) -> Result<String, String> {
    let snap_filter = match snapshot_id {
        Some(id) => format!("AND snapshot_id = {}", id),
        None => "AND snapshot_id = (SELECT MAX(id) FROM graph_snapshots)".to_string(),
    };

    let simple_sql = format!(
        "SELECT concept, count, domain
         FROM graph_nodes
         WHERE 1=1 {}
         ORDER BY count DESC
         LIMIT {}",
        snap_filter, limit
    );

    db::query_sexp(&simple_sql)
}

/// Query: graph evolution over time — how the graph grows/shrinks.
pub fn graph_evolution(since_ts: i64) -> Result<String, String> {
    let sql = format!(
        "SELECT ts, source, node_count, edge_count, interdisciplinary_edges
         FROM graph_snapshots
         WHERE ts >= {}
         ORDER BY ts",
        since_ts
    );
    db::query_sexp(&sql)
}
