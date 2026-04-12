//! Service implementation — pure handler + atomic apply.

use harmonia_actor_protocol::{MemoryError, Service};
use crate::command::*;
use crate::FieldState;

impl Service for FieldState {
    type Cmd = FieldCommand;
    type Ok = FieldResult;
    type Delta = FieldDelta;

    fn handle(&self, cmd: FieldCommand) -> Result<(FieldDelta, FieldResult), MemoryError> {
        match cmd {
            FieldCommand::Status => {
                let result = FieldResult::Status(StatusResult {
                    cycle: self.cycle,
                    graph_n: self.graph.n,
                    graph_version: self.graph_version,
                    spectral_k: self.eigenvalues.len(),
                    basin: self.hysteresis.current_basin.to_sexp().to_string(),
                    thomas_b: self.thomas_b,
                });
                Ok((FieldDelta::None, result))
            }
            FieldCommand::BasinStatus => {
                let result = FieldResult::BasinStatus(BasinStatusResult {
                    current: self.hysteresis.current_basin.to_sexp().to_string(),
                    dwell_ticks: self.hysteresis.dwell_ticks,
                    coercive_energy: self.hysteresis.coercive_energy,
                    threshold: self.hysteresis.threshold,
                });
                Ok((FieldDelta::None, result))
            }
            FieldCommand::EigenmodeStatus => {
                let result = FieldResult::EigenmodeStatus(EigenmodeResult {
                    eigenvalues: self.eigenvalues.clone(),
                    spectral_version: self.spectral_version,
                    graph_version: self.graph_version,
                });
                Ok((FieldDelta::None, result))
            }
            FieldCommand::DreamStats => {
                let result = FieldResult::DreamStats(DreamStatsResult {
                    dreams: self.dream_count,
                    pruned: self.total_pruned,
                    merged: self.total_merged,
                    crystallized: self.total_crystallized,
                    entropy_delta: self.cumulative_entropy_delta,
                });
                Ok((FieldDelta::None, result))
            }
            FieldCommand::CurrentBasin => {
                Ok((FieldDelta::None, FieldResult::CurrentBasin {
                    basin: self.hysteresis.current_basin.to_sexp().to_string(),
                    cycle: self.cycle,
                }))
            }
            FieldCommand::Recall { query_concepts, access_counts, limit } => {
                let result = crate::recall::compute_recall_pure(self, &query_concepts, &access_counts, limit);
                let new_cycle = self.cycle + 1;
                Ok((FieldDelta::CycleIncremented { new_cycle }, FieldResult::Recalled(result)))
            }
            FieldCommand::RecallStructural { query_concepts, limit } => {
                let result = crate::recall::compute_recall_pure(self, &query_concepts, &[], limit);
                let new_cycle = self.cycle + 1;
                Ok((FieldDelta::CycleIncremented { new_cycle }, FieldResult::Recalled(result)))
            }
            FieldCommand::LoadGraph { nodes, edges, directed_weights } => {
                let (mut graph, eigenvalues, eigenvectors, node_basins) =
                    crate::api::compute_load_graph(&nodes, &edges, &self.thomas, &self.aizawa, &self.halvorsen);
                // Populate directed weights for A-B topological flux.
                if !directed_weights.is_empty() {
                    crate::graph::set_directed_weights(&mut graph, &directed_weights);
                }
                let n = graph.n;
                let edge_count = graph.col_idx.len() / 2;
                let spectral_k = eigenvalues.len();
                let new_version = self.graph_version + 1;
                let directed = if graph.directed_weights.is_empty() { None } else { Some(&graph.directed_weights) };
                let topology = crate::topology::compute_topology(&graph, directed);
                Ok((
                    FieldDelta::GraphRebuilt {
                        graph, eigenvalues, eigenvectors,
                        graph_version: new_version,
                        spectral_version: new_version,
                        node_basins,
                        topology,
                    },
                    FieldResult::GraphLoaded { n, edges: edge_count, spectral_k, graph_version: new_version },
                ))
            }
            FieldCommand::StepAttractors { signal, noise } => {
                let (thomas, thomas_b, aizawa, halvorsen, hysteresis, node_basins, thomas_measure, thomas_soft_basins) =
                    crate::attractor_api::compute_step_pure(self, signal, noise);
                // Extract coordinates before moving into the delta.
                let thomas_coords = (thomas.x, thomas.y, thomas.z);
                let aizawa_coords = (aizawa.x, aizawa.y, aizawa.z);
                let halvorsen_coords = (halvorsen.x, halvorsen.y, halvorsen.z);
                let basin_str = hysteresis.current_basin.to_sexp().to_string();
                Ok((
                    FieldDelta::AttractorStepped { thomas, thomas_b, aizawa, halvorsen, hysteresis, node_basins, thomas_measure, thomas_soft_basins, last_signal: signal, last_noise: noise },
                    FieldResult::Stepped(SteppedResult {
                        thomas: thomas_coords,
                        thomas_b,
                        aizawa: aizawa_coords,
                        halvorsen: halvorsen_coords,
                        basin: basin_str,
                    }),
                ))
            }
            FieldCommand::Dream => {
                let report = crate::dream::compute_dream_pure(self)?;
                Ok((
                    FieldDelta::DreamCompleted {
                        entropy_delta: report.entropy_delta,
                        pruned_count: report.pruned_entries.len() as u64,
                        merged_count: report.merge_groups.len() as u64,
                        crystallized_count: report.crystallized_entries.len() as u64,
                    },
                    FieldResult::Dreamed(report),
                ))
            }
            FieldCommand::EdgeCurrents => {
                let currents = crate::api::compute_edge_currents_pure(self);
                Ok((FieldDelta::None, FieldResult::EdgeCurrents(currents)))
            }
            FieldCommand::Digest => {
                let digest = crate::api::compute_digest_pure(self);
                Ok((FieldDelta::None, FieldResult::Digest(digest)))
            }
            FieldCommand::RestoreBasin { basin_str, coercive_energy, dwell_ticks, threshold } => {
                let basin = crate::basin::Basin::from_sexp(&basin_str);
                let hysteresis = crate::basin::HysteresisTracker::restored(basin, coercive_energy, dwell_ticks, threshold);
                let basin_name = hysteresis.current_basin.to_sexp().to_string();
                Ok((
                    FieldDelta::BasinRestored { hysteresis },
                    FieldResult::BasinRestored(BasinRestoredResult {
                        basin: basin_name, energy: coercive_energy, dwell: dwell_ticks, threshold,
                    }),
                ))
            }
            FieldCommand::LoadGenesis { entries } => {
                let (all_nodes, all_edges) = crate::api::flatten_genesis_entries(&entries);
                let (graph, eigenvalues, eigenvectors, node_basins) =
                    crate::api::compute_load_graph(&all_nodes, &all_edges, &self.thomas, &self.aizawa, &self.halvorsen);
                let n = graph.n;
                let edge_count = graph.col_idx.len() / 2;
                let spectral_k = eigenvalues.len();
                let new_version = self.graph_version + 1;
                let topology = crate::topology::compute_topology(&graph, None);
                Ok((
                    FieldDelta::GraphRebuilt {
                        graph, eigenvalues, eigenvectors,
                        graph_version: new_version,
                        spectral_version: new_version,
                        node_basins,
                        topology,
                    },
                    FieldResult::GenesisLoaded { n, edges: edge_count, spectral_k, graph_version: new_version },
                ))
            }
            FieldCommand::Bootstrap => {
                if self.graph.n == 0 {
                    return Err(MemoryError::GraphEmpty);
                }

                // Step 1: step attractors (pure).
                let (thomas, thomas_b, aizawa, halvorsen, hysteresis, node_basins, thomas_measure, thomas_soft_basins) =
                    crate::attractor_api::compute_step_pure(self, 0.5, 0.1);

                // Step 2: dream (pure) — use current graph state.
                let dream_report = crate::dream::compute_dream_pure(self)?;

                let basin_str = hysteresis.current_basin.to_sexp().to_string();
                Ok((
                    FieldDelta::AttractorStepped { thomas, thomas_b, aizawa, halvorsen, hysteresis, node_basins, thomas_measure, thomas_soft_basins, last_signal: 0.5, last_noise: 0.1 },
                    FieldResult::Bootstrapped(BootstrapResult {
                        nodes: self.graph.n,
                        basin: basin_str,
                        dream: dream_report,
                    }),
                ))
            }
            FieldCommand::Reset => {
                Ok((FieldDelta::Reset, FieldResult::Reset))
            }
            FieldCommand::Checkpoint => {
                let sexp = self.checkpoint_sexp();
                Ok((FieldDelta::None, FieldResult::Checkpointed { sexp }))
            }
            FieldCommand::RestoreState { thomas, aizawa, halvorsen, signal, noise, soft_basins, thomas_b } => {
                use crate::attractor::{ThomasState, AizawaState, HalvorsenState};
                Ok((
                    FieldDelta::StateRestored {
                        thomas: ThomasState { x: thomas.0, y: thomas.1, z: thomas.2 },
                        aizawa: AizawaState { x: aizawa.0, y: aizawa.1, z: aizawa.2 },
                        halvorsen: HalvorsenState { x: halvorsen.0, y: halvorsen.1, z: halvorsen.2 },
                        signal, noise, soft_basins, thomas_b,
                    },
                    FieldResult::StateRestored,
                ))
            }
            FieldCommand::SaveToDisk => {
                let sexp = match self.save_to_disk() {
                    Ok(()) => "(:ok :saved t)".to_string(),
                    Err(e) => format!("(:error \"{}\")", e),
                };
                Ok((FieldDelta::None, FieldResult::DiskSaved { sexp }))
            }
            FieldCommand::LoadFromDisk => {
                // LoadFromDisk is a read-only probe here; actual mutation happens
                // outside the Service pattern since handle() takes &self.
                // Return whether a state file exists so the caller can restore.
                let restored = match crate::field_state_dir() {
                    Some(dir) => dir.join("state.sexp").exists(),
                    None => false,
                };
                Ok((FieldDelta::None, FieldResult::DiskLoaded { restored }))
            }
        }
    }

    fn apply(&mut self, delta: FieldDelta) {
        match delta {
            FieldDelta::None => {}
            FieldDelta::GraphRebuilt { graph, eigenvalues, eigenvectors, graph_version, spectral_version, node_basins, topology } => {
                self.graph = graph;
                self.eigenvalues = eigenvalues;
                self.eigenvectors = eigenvectors;
                self.graph_version = graph_version;
                self.spectral_version = spectral_version;
                self.node_basins = node_basins;
                self.topology = topology;
            }
            FieldDelta::AttractorStepped { thomas, thomas_b, aizawa, halvorsen, hysteresis, node_basins, thomas_measure, thomas_soft_basins, last_signal, last_noise } => {
                self.thomas = thomas;
                self.thomas_b = thomas_b;
                self.aizawa = aizawa;
                self.halvorsen = halvorsen;
                self.hysteresis = hysteresis;
                self.node_basins = node_basins;
                self.thomas_measure = thomas_measure;
                self.thomas_soft_basins = thomas_soft_basins;
                self.last_signal = last_signal;
                self.last_noise = last_noise;
            }
            FieldDelta::DreamCompleted { entropy_delta, pruned_count, merged_count, crystallized_count } => {
                self.cumulative_entropy_delta += entropy_delta;
                self.dream_count += 1;
                self.total_pruned += pruned_count;
                self.total_merged += merged_count;
                self.total_crystallized += crystallized_count;
            }
            FieldDelta::CycleIncremented { new_cycle } => {
                self.cycle = new_cycle;
            }
            FieldDelta::BasinRestored { hysteresis } => {
                self.hysteresis = hysteresis;
            }
            FieldDelta::StateRestored { thomas, aizawa, halvorsen, signal, noise, soft_basins, thomas_b } => {
                self.thomas = thomas;
                self.aizawa = aizawa;
                self.halvorsen = halvorsen;
                self.last_signal = signal;
                self.last_noise = noise;
                self.thomas_soft_basins = soft_basins;
                self.thomas_b = thomas_b;
            }
            FieldDelta::Reset => {
                *self = FieldState::new();
            }
        }
    }
}
