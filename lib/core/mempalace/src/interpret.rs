//! Service implementation -- pure handler + atomic apply.
//!
//! Follows the Free Monad pattern from actor-protocol:
//! handle() is PURE (immutable &self), apply() is the ONE mutation point.

use harmonia_actor_protocol::{MemoryError, Service};

use crate::command::*;
use crate::PalaceState;

impl Service for PalaceState {
    type Cmd = PalaceCommand;
    type Ok = PalaceResult;
    type Delta = PalaceDelta;

    fn handle(&self, cmd: PalaceCommand) -> Result<(PalaceDelta, PalaceResult), MemoryError> {
        match cmd {
            PalaceCommand::FileDrawer {
                content,
                room_id,
                source,
                tags,
            } => {
                // Pure computation: validate, create drawer, compute disk descriptor.
                if content.len() < 10 {
                    return Err(MemoryError::InvalidContent("min 10 chars".into()));
                }
                if self.graph.find_node_by_id(room_id).is_none() {
                    return Err(MemoryError::RoomNotFound(room_id));
                }
                let id = self.next_drawer_id;
                let drawer = crate::drawer::Drawer {
                    id,
                    content: content.clone(),
                    source,
                    room_id,
                    chunk_index: 0,
                    created_at: crate::current_epoch_ms(),
                    tags,
                };

                // Compute disk write descriptor (pure -- no I/O yet)
                let disk_write = crate::disk::memory_root().map(|root| {
                    let (wing_label, room_label) = self.resolve_wing_room(room_id);
                    let path =
                        crate::disk::drawer_md_path(&root, &wing_label, &room_label, id);
                    let md = crate::disk::drawer_to_md(&drawer);
                    (path, md)
                });

                let size = content.len();
                Ok((
                    PalaceDelta::DrawerAdded { drawer, disk_write },
                    PalaceResult::DrawerFiled { id, room_id, size },
                ))
            }

            PalaceCommand::GetDrawer { id } => match self.drawers.get(id) {
                Some(d) => {
                    let sexp = format!(
                        "(:ok :id {} :room {} :content \"{}\" :source {} :tags ({}) :created {})",
                        d.id,
                        d.room_id,
                        crate::sexp_escape(&d.content),
                        d.source.to_sexp(),
                        d.tags
                            .iter()
                            .map(|t| format!("\"{}\"", crate::sexp_escape(t)))
                            .collect::<Vec<_>>()
                            .join(" "),
                        d.created_at,
                    );
                    Ok((PalaceDelta::None, PalaceResult::DrawerRetrieved(sexp)))
                }
                None => Err(MemoryError::DrawerNotFound(id)),
            },

            PalaceCommand::Search {
                query,
                room_filter,
                limit,
            } => {
                let limit = limit.min(50).max(1);
                let results = self.drawers.search(&query, room_filter, limit);
                let items: Vec<String> = results
                    .iter()
                    .map(|d| {
                        let preview = if d.content.len() > 120 {
                            format!("{}...", crate::truncate_safe(&d.content, 120))
                        } else {
                            d.content.clone()
                        };
                        format!(
                            "(:id {} :room {} :preview \"{}\" :tags ({}))",
                            d.id,
                            d.room_id,
                            crate::sexp_escape(&preview),
                            d.tags
                                .iter()
                                .map(|t| format!("\"{}\"", crate::sexp_escape(t)))
                                .collect::<Vec<_>>()
                                .join(" "),
                        )
                    })
                    .collect();
                let sexp = format!(
                    "(:ok :count {} :results ({}))",
                    results.len(),
                    items.join(" ")
                );
                Ok((PalaceDelta::None, PalaceResult::SearchResults(sexp)))
            }

            PalaceCommand::AddNode {
                kind,
                label,
                domain,
            } => {
                let max_nodes = crate::cfg_usize("max-nodes", 1024);
                if self.graph.nodes.len() >= max_nodes {
                    return Err(MemoryError::CapacityExceeded {
                        kind: "nodes",
                        limit: max_nodes,
                    });
                }
                if self.graph.find_node(&label).is_some() {
                    return Err(MemoryError::DuplicateNode(label));
                }
                let id = self.graph.nodes.len() as u32;
                let node = crate::graph::GraphNode {
                    id,
                    kind,
                    label: label.clone(),
                    domain,
                    created_at: crate::current_epoch_ms(),
                    properties: Vec::new(),
                };
                Ok((
                    PalaceDelta::NodeAdded { node },
                    PalaceResult::NodeAdded { id, label },
                ))
            }

            PalaceCommand::AddEdge {
                source,
                target,
                kind,
                weight,
            } => {
                let n = self.graph.nodes.len();
                if source as usize >= n || target as usize >= n {
                    return Err(MemoryError::NodeNotFound(format!(
                        "id {source} or {target}"
                    )));
                }
                let edge = crate::graph::GraphEdge {
                    source,
                    target,
                    kind,
                    weight: weight.clamp(0.0, 1.0),
                    valid_from: crate::current_epoch_ms(),
                    valid_to: None,
                    confidence: 1.0,
                };
                Ok((
                    PalaceDelta::EdgeAdded { edge },
                    PalaceResult::EdgeAdded { source, target },
                ))
            }

            PalaceCommand::Persist => {
                // Pure: compute all write descriptors
                let mut drawer_writes = Vec::new();
                if let Some(root) = crate::disk::memory_root() {
                    for d in self.drawers.all() {
                        let (wing, room) = self.resolve_wing_room(d.room_id);
                        let path = crate::disk::drawer_md_path(&root, &wing, &room, d.id);
                        let md = crate::disk::drawer_to_md(d);
                        drawer_writes.push((path, md));
                    }
                    let graph_path = root.join("palace").join("index.sexp");
                    let graph_sexp = crate::disk::graph_to_sexp(&self.graph);
                    let codebook_path = root.join("codebook.json");
                    let codebook_json = self.codebook.to_json();

                    let n_drawers = self.drawers.len();
                    let n_nodes = self.graph.nodes.len();
                    let n_edges = self.graph.edges.len();

                    Ok((
                        PalaceDelta::Persisted {
                            graph_sexp,
                            graph_path,
                            codebook_json,
                            codebook_path,
                            drawer_writes,
                        },
                        PalaceResult::Persisted {
                            drawers: n_drawers,
                            nodes: n_nodes,
                            edges: n_edges,
                        },
                    ))
                } else {
                    // No memory root configured -- no disk writes, just counts.
                    Ok((
                        PalaceDelta::None,
                        PalaceResult::Persisted {
                            drawers: self.drawers.len(),
                            nodes: self.graph.nodes.len(),
                            edges: self.graph.edges.len(),
                        },
                    ))
                }
            }

            PalaceCommand::Init => {
                // Pure: read from disk and prepare the Restored delta.
                if let Some(root) = crate::disk::memory_root() {
                    let disk_drawers = crate::disk::load_all_drawers(&root);

                    let mut drawers = Vec::new();
                    let mut next_id = 1u64;
                    for (id, content, source, room_id, chunk_index, created_at, tags) in
                        disk_drawers
                    {
                        drawers.push(crate::drawer::Drawer {
                            id,
                            content,
                            source,
                            room_id,
                            chunk_index,
                            created_at,
                            tags,
                        });
                        next_id = next_id.max(id + 1);
                    }

                    let mut nodes = Vec::new();
                    let mut edges = Vec::new();
                    let graph_path = root.join("palace").join("index.sexp");
                    if let Some((raw_nodes, raw_edges)) = crate::disk::load_graph_index(&graph_path)
                    {
                        for (id, kind, label, domain, created_at) in raw_nodes {
                            nodes.push(crate::graph::GraphNode {
                                id,
                                kind: crate::graph::NodeKind::from_str(&kind),
                                label,
                                domain: crate::graph::Domain::from_str(&domain),
                                created_at,
                                properties: Vec::new(),
                            });
                        }
                        for (source, target, kind, weight, confidence) in raw_edges {
                            edges.push(crate::graph::GraphEdge {
                                source,
                                target,
                                kind: crate::graph::EdgeKind::from_str(&kind),
                                weight,
                                valid_from: 0,
                                valid_to: None,
                                confidence,
                            });
                        }
                    }

                    // Codebook: disk first, config-store fallback
                    let cb_path = root.join("codebook.json");
                    let codebook = if let Ok(json) = std::fs::read_to_string(&cb_path) {
                        crate::codebook::AaakCodebook::from_json(&json)
                    } else if let Ok(Some(json)) =
                        harmonia_config_store::get_own("mempalace", "codebook")
                    {
                        crate::codebook::AaakCodebook::from_json(&json)
                    } else {
                        crate::codebook::AaakCodebook::new()
                    };

                    let n_nodes = nodes.len();
                    let n_drawers = drawers.len();
                    let n_codebook = codebook.len();

                    Ok((
                        PalaceDelta::Restored {
                            drawers,
                            nodes,
                            edges,
                            codebook,
                            next_drawer_id: next_id,
                        },
                        PalaceResult::Initialized {
                            nodes: n_nodes,
                            drawers: n_drawers,
                            codebook: n_codebook,
                        },
                    ))
                } else {
                    // No memory root -- return empty init result
                    Ok((
                        PalaceDelta::None,
                        PalaceResult::Initialized {
                            nodes: 0,
                            drawers: 0,
                            codebook: 0,
                        },
                    ))
                }
            }
        }
    }

    fn apply(&mut self, delta: PalaceDelta) {
        match delta {
            PalaceDelta::None => {}

            PalaceDelta::DrawerAdded { drawer, disk_write } => {
                self.next_drawer_id = self.next_drawer_id.max(drawer.id + 1);
                self.drawers.push(drawer);
                // Disk I/O -- the ONLY side effect, in apply()
                if let Some((path, content)) = disk_write {
                    let _ = crate::disk::write_drawer_md(&path, &content);
                }
            }

            PalaceDelta::NodeAdded { node } => {
                self.graph.nodes.push(node);
                self.graph
                    .offsets
                    .push(*self.graph.offsets.last().unwrap_or(&0));
            }

            PalaceDelta::EdgeAdded { edge } => {
                self.graph.edges.push(edge);
                self.graph.rebuild_csr();
            }

            PalaceDelta::Persisted {
                graph_sexp,
                graph_path,
                codebook_json,
                codebook_path,
                drawer_writes,
            } => {
                // Write all drawer .md files
                for (path, content) in &drawer_writes {
                    let _ = crate::disk::write_drawer_md(path, content);
                }
                // Write graph index (atomic)
                if let Some(parent) = graph_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let tmp = graph_path.with_extension("sexp.tmp");
                let _ = std::fs::write(&tmp, &graph_sexp);
                let _ = std::fs::rename(&tmp, &graph_path);
                // Write codebook (atomic)
                if let Some(parent) = codebook_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let tmp = codebook_path.with_extension("json.tmp");
                let _ = std::fs::write(&tmp, &codebook_json);
                let _ = std::fs::rename(&tmp, &codebook_path);
            }

            PalaceDelta::Restored {
                drawers,
                nodes,
                edges,
                codebook,
                next_drawer_id,
            } => {
                for d in drawers {
                    self.drawers.push(d);
                }
                for n in nodes {
                    self.graph.nodes.push(n);
                    self.graph
                        .offsets
                        .push(*self.graph.offsets.last().unwrap_or(&0));
                }
                for e in edges {
                    self.graph.edges.push(e);
                }
                if !self.graph.edges.is_empty() {
                    self.graph.rebuild_csr();
                }
                self.codebook = codebook;
                self.next_drawer_id = next_drawer_id;
            }
        }
    }
}
