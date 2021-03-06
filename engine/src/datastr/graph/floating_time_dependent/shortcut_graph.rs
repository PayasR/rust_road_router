//! Graph structs used during and after the customization.

use super::*;
use crate::datastr::clearlist_vector::ClearlistVector;
use crate::datastr::graph::first_out_graph::degrees_to_first_out;
use crate::datastr::rank_select_map::*;
use crate::io::*;
use crate::util::*;
use std::cmp::min;

/// Container for partial CCH graphs during CATCHUp customization.
/// Think split borrows.
#[derive(Debug)]
pub struct PartialShortcutGraph<'a> {
    pub original_graph: &'a TDGraph,
    outgoing: &'a [Shortcut],
    incoming: &'a [Shortcut],
    offset: usize,
}

impl<'a> PartialShortcutGraph<'a> {
    /// Create `PartialShortcutGraph` from original graph, shortcut slices in both directions and an offset to map CCH edge ids to slice indices
    pub fn new(original_graph: &'a TDGraph, outgoing: &'a [Shortcut], incoming: &'a [Shortcut], offset: usize) -> PartialShortcutGraph<'a> {
        PartialShortcutGraph {
            original_graph,
            outgoing,
            incoming,
            offset,
        }
    }

    /// Borrow upward `Shortcut` with given CCH EdgeId
    pub fn get_outgoing(&self, edge_id: EdgeId) -> &Shortcut {
        &self.outgoing[edge_id as usize - self.offset]
    }

    /// Borrow downward `Shortcut` with given CCH EdgeId
    pub fn get_incoming(&self, edge_id: EdgeId) -> &Shortcut {
        &self.incoming[edge_id as usize - self.offset]
    }
}

// Just a container to group some data
#[derive(Debug)]
struct ShortcutGraph<'a> {
    original_graph: &'a TDGraph,
    first_out: &'a [EdgeId],
    head: &'a [NodeId],
    outgoing: Vec<Shortcut>,
    incoming: Vec<Shortcut>,
}

/// Result of CATCHUp customization to be passed to query algorithm.
#[derive(Debug)]
pub struct CustomizedGraph<'a> {
    pub original_graph: &'a TDGraph,
    first_out: &'a [EdgeId],
    head: &'a [NodeId],
    pub outgoing: CustomizedSingleDirGraph,
    pub incoming: CustomizedSingleDirGraph,
}

impl<'a> From<ShortcutGraph<'a>> for CustomizedGraph<'a> {
    // cleaning up and compacting preprocessing results.
    fn from(shortcut_graph: ShortcutGraph<'a>) -> Self {
        let mut outgoing_required = BitVec::new(shortcut_graph.head.len());
        let mut incoming_required = BitVec::new(shortcut_graph.head.len());

        for (idx, s) in shortcut_graph.outgoing.iter().enumerate() {
            if s.required {
                outgoing_required.set(idx)
            }
        }

        for (idx, s) in shortcut_graph.incoming.iter().enumerate() {
            if s.required {
                incoming_required.set(idx)
            }
        }

        let mapping_outgoing = RankSelectMap::new(outgoing_required);
        let mapping_incoming = RankSelectMap::new(incoming_required);

        let mut outgoing_first_out = Vec::with_capacity(shortcut_graph.first_out.len());
        let mut incoming_first_out = Vec::with_capacity(shortcut_graph.first_out.len());
        let mut outgoing_head = Vec::with_capacity(shortcut_graph.head.len());
        let mut incoming_head = Vec::with_capacity(shortcut_graph.head.len());

        outgoing_first_out.push(0);
        incoming_first_out.push(0);

        for range in shortcut_graph.first_out.windows(2) {
            let range = range[0] as usize..range[1] as usize;
            outgoing_head.extend(
                shortcut_graph.head[range.clone()]
                    .iter()
                    .zip(shortcut_graph.outgoing[range.clone()].iter())
                    .filter(|(_head, s)| s.required)
                    .map(|(head, _)| head),
            );
            outgoing_first_out.push(outgoing_first_out.last().unwrap() + shortcut_graph.outgoing[range.clone()].iter().filter(|s| s.required).count() as u32);

            incoming_head.extend(
                shortcut_graph.head[range.clone()]
                    .iter()
                    .zip(shortcut_graph.incoming[range.clone()].iter())
                    .filter(|(_head, s)| s.required)
                    .map(|(head, _)| head),
            );
            incoming_first_out.push(incoming_first_out.last().unwrap() + shortcut_graph.incoming[range.clone()].iter().filter(|s| s.required).count() as u32);
        }

        let mut outgoing_constant = BitVec::new(outgoing_head.len());
        let mut incoming_constant = BitVec::new(incoming_head.len());

        let outgoing_iter = || shortcut_graph.outgoing.iter().filter(|s| s.required);
        let incoming_iter = || shortcut_graph.incoming.iter().filter(|s| s.required);

        for (idx, shortcut) in outgoing_iter().enumerate() {
            if shortcut.is_constant() {
                outgoing_constant.set(idx);
            }
        }

        for (idx, shortcut) in incoming_iter().enumerate() {
            if shortcut.is_constant() {
                incoming_constant.set(idx);
            }
        }

        let mut outgoing_tail = vec![0 as NodeId; outgoing_head.len()];
        for (node, range) in outgoing_first_out.windows(2).enumerate() {
            for tail in &mut outgoing_tail[range[0] as usize..range[1] as usize] {
                *tail = node as NodeId;
            }
        }

        let mut incoming_tail = vec![0 as NodeId; incoming_head.len()];
        for (node, range) in incoming_first_out.windows(2).enumerate() {
            for tail in &mut incoming_tail[range[0] as usize..range[1] as usize] {
                *tail = node as NodeId;
            }
        }

        CustomizedGraph {
            original_graph: shortcut_graph.original_graph,
            first_out: shortcut_graph.first_out,
            head: shortcut_graph.head,

            outgoing: CustomizedSingleDirGraph {
                first_out: outgoing_first_out,
                head: outgoing_head,
                tail: outgoing_tail,

                bounds: outgoing_iter().map(|shortcut| (shortcut.lower_bound, shortcut.upper_bound)).collect(),
                constant: outgoing_constant,
                first_source: degrees_to_first_out(outgoing_iter().map(|shortcut| shortcut.num_sources() as u32)).collect(),
                sources: outgoing_iter()
                    .flat_map(|shortcut| {
                        shortcut.sources_iter().map(|(t, &s)| {
                            let s = if let ShortcutSource::Shortcut(down, up) = ShortcutSource::from(s) {
                                ShortcutSource::Shortcut(
                                    mapping_incoming.get(down as usize).unwrap() as EdgeId,
                                    mapping_outgoing.get(up as usize).unwrap() as EdgeId,
                                )
                            } else {
                                ShortcutSource::from(s)
                            };
                            (t, ShortcutSourceData::from(s))
                        })
                    })
                    .collect(),
            },

            incoming: CustomizedSingleDirGraph {
                first_out: incoming_first_out,
                head: incoming_head,
                tail: incoming_tail,

                bounds: incoming_iter().map(|shortcut| (shortcut.lower_bound, shortcut.upper_bound)).collect(),
                constant: incoming_constant,
                first_source: degrees_to_first_out(incoming_iter().map(|shortcut| shortcut.num_sources() as u32)).collect(),
                sources: incoming_iter()
                    .flat_map(|shortcut| {
                        shortcut.sources_iter().map(|(t, &s)| {
                            let s = if let ShortcutSource::Shortcut(down, up) = ShortcutSource::from(s) {
                                ShortcutSource::Shortcut(
                                    mapping_incoming.get(down as usize).unwrap() as EdgeId,
                                    mapping_outgoing.get(up as usize).unwrap() as EdgeId,
                                )
                            } else {
                                ShortcutSource::from(s)
                            };
                            (t, ShortcutSourceData::from(s))
                        })
                    })
                    .collect(),
            },
        }
    }
}

impl<'a> CustomizedGraph<'a> {
    /// Create CustomizedGraph from original graph, CCH topology, and customized `Shortcut`s for each CCH edge in both directions
    pub fn new(original_graph: &'a TDGraph, first_out: &'a [EdgeId], head: &'a [NodeId], outgoing: Vec<Shortcut>, incoming: Vec<Shortcut>) -> Self {
        ShortcutGraph {
            original_graph,
            first_out,
            head,
            outgoing,
            incoming,
        }
        .into()
    }

    /// Get bounds graph for forward elimination tree interval query
    pub fn upward_bounds_graph(&self) -> SingleDirBoundsGraph {
        SingleDirBoundsGraph {
            first_out: &self.outgoing.first_out[..],
            head: &self.outgoing.head[..],
            bounds: &self.outgoing.bounds[..],
        }
    }

    /// Get bounds graph for backward elimination tree interval query
    pub fn downward_bounds_graph(&self) -> SingleDirBoundsGraph {
        SingleDirBoundsGraph {
            first_out: &self.incoming.first_out[..],
            head: &self.incoming.head[..],
            bounds: &self.incoming.bounds[..],
        }
    }
}

impl<'a> Deconstruct for CustomizedGraph<'a> {
    fn store_each(&self, store: &dyn Fn(&str, &dyn Store) -> std::io::Result<()>) -> std::io::Result<()> {
        store("outgoing_first_out", &self.outgoing.first_out)?;
        store("outgoing_head", &self.outgoing.head)?;
        store("outgoing_bounds", &self.outgoing.bounds)?;
        store("outgoing_constant", &self.outgoing.constant)?;
        store("outgoing_first_source", &self.outgoing.first_source)?;
        store("outgoing_sources", &self.outgoing.sources)?;
        store("incoming_first_out", &self.incoming.first_out)?;
        store("incoming_head", &self.incoming.head)?;
        store("incoming_bounds", &self.incoming.bounds)?;
        store("incoming_constant", &self.incoming.constant)?;
        store("incoming_first_source", &self.incoming.first_source)?;
        store("incoming_sources", &self.incoming.sources)?;
        Ok(())
    }
}

/// Additional data to load CATCHUp customization results back from disk.
#[derive(Debug)]
pub struct CustomizedGraphReconstrctor<'a> {
    pub original_graph: &'a TDGraph,
    pub first_out: &'a [EdgeId],
    pub head: &'a [NodeId],
}

impl<'a> ReconstructPrepared<CustomizedGraph<'a>> for CustomizedGraphReconstrctor<'a> {
    fn reconstruct_with(self, loader: Loader) -> std::io::Result<CustomizedGraph<'a>> {
        let outgoing_first_out: Vec<EdgeId> = loader.load("outgoing_first_out")?;
        let outgoing_head: Vec<NodeId> = loader.load("outgoing_head")?;
        let incoming_first_out: Vec<EdgeId> = loader.load("incoming_first_out")?;
        let incoming_head: Vec<NodeId> = loader.load("incoming_head")?;

        let mut outgoing_tail = vec![0 as NodeId; outgoing_head.len()];
        for (node, range) in outgoing_first_out.windows(2).enumerate() {
            for tail in &mut outgoing_tail[range[0] as usize..range[1] as usize] {
                *tail = node as NodeId;
            }
        }

        let mut incoming_tail = vec![0 as NodeId; incoming_head.len()];
        for (node, range) in incoming_first_out.windows(2).enumerate() {
            for tail in &mut incoming_tail[range[0] as usize..range[1] as usize] {
                *tail = node as NodeId;
            }
        }

        Ok(CustomizedGraph {
            original_graph: self.original_graph,
            first_out: self.first_out,
            head: self.head,

            outgoing: CustomizedSingleDirGraph {
                first_out: outgoing_first_out,
                head: outgoing_head,
                tail: outgoing_tail,

                bounds: loader.load("outgoing_bounds")?,
                constant: loader.load("outgoing_constant")?,
                first_source: loader.load("outgoing_first_source")?,
                sources: loader.load("outgoing_sources")?,
            },

            incoming: CustomizedSingleDirGraph {
                first_out: incoming_first_out,
                head: incoming_head,
                tail: incoming_tail,

                bounds: loader.load("incoming_bounds")?,
                constant: loader.load("incoming_constant")?,
                first_source: loader.load("incoming_first_source")?,
                sources: loader.load("incoming_sources")?,
            },
        })
    }
}

/// Data for result of CATCHUp customization; one half/direction of it.
#[derive(Debug)]
pub struct CustomizedSingleDirGraph {
    first_out: Vec<EdgeId>,
    head: Vec<NodeId>,
    tail: Vec<NodeId>,

    bounds: Vec<(FlWeight, FlWeight)>,
    constant: BitVec,
    first_source: Vec<u32>,
    sources: Vec<(Timestamp, ShortcutSourceData)>,
}

impl CustomizedSingleDirGraph {
    /// Number of outgoing/incoming edges to/from higher ranked nodes for a given node
    pub fn degree(&self, node: NodeId) -> usize {
        (self.first_out[node as usize + 1] - self.first_out[node as usize]) as usize
    }

    /// Borrow full slice of upper and lower bounds for each edge in this graph
    pub fn bounds(&self) -> &[(FlWeight, FlWeight)] {
        &self.bounds[..]
    }

    /// Borrow full slice of head node for each edge in this graph
    pub fn head(&self) -> &[NodeId] {
        &self.head[..]
    }

    /// Borrow full slice of tail node for each edge in this graph
    pub fn tail(&self) -> &[NodeId] {
        &self.tail[..]
    }

    /// (Recursively) evaluate the travel time of edge with a given id for given point in time.
    /// The callback `f` can be used to do early returns if we reach a node that already has a better tentative distance.
    pub fn evaluate<F>(&self, edge_id: EdgeId, t: Timestamp, customized_graph: &CustomizedGraph, f: &mut F) -> FlWeight
    where
        F: (FnMut(bool, EdgeId, Timestamp) -> bool),
    {
        let edge_idx = edge_id as usize;
        if self.constant.get(edge_idx) {
            debug_assert_eq!(
                self.bounds[edge_idx].0,
                self.edge_source_at(edge_idx, t)
                    .map(|&source| ShortcutSource::from(source).evaluate(t, customized_graph, &mut always))
                    .unwrap_or(FlWeight::INFINITY),
                "{:?}, {:?}, {}",
                self.bounds[edge_idx],
                self.edge_sources(edge_idx),
                edge_id
            );
            return self.bounds[edge_idx].0;
        }

        self.edge_source_at(edge_idx, t)
            .map(|&source| ShortcutSource::from(source).evaluate(t, customized_graph, f))
            .unwrap_or(FlWeight::INFINITY)
    }

    /// Evaluate the first original edge on the path that the edge with the given id represents at the given point in time.
    ///
    /// This means we recursively unpack the downward edges of all lower triangles of shortcuts.
    /// While doing so, we mark the respective up arc as contained in the search space using the `mark_upwards` callback.
    /// We also update lower bounds to the target of all middle nodes of unpacked triangles.
    /// The Dir parameter is used to distinguish the direction of the current edge - True means upward, False downward.
    /// We return an `Option` of a tuple with the evaluated `FlWeight`, the CCH `NodeId` of the head node of the evaluated edge, and the CCH `EdgeId` of the evaluated edge.
    /// The result will be `None` when this is an always infinity edge.
    pub fn evaluate_next_segment_at<Dir: Bool, F>(
        &self,
        edge_id: EdgeId,
        t: Timestamp,
        lower_bound_target: FlWeight,
        customized_graph: &CustomizedGraph,
        lower_bounds_to_target: &mut ClearlistVector<FlWeight>,
        mark_upward: &mut F,
    ) -> Option<(FlWeight, NodeId, EdgeId)>
    where
        F: FnMut(EdgeId),
    {
        let edge_idx = edge_id as usize;

        if self.constant.get(edge_idx) {
            return Some((
                self.bounds[edge_idx].0,
                if Dir::VALUE { self.head[edge_idx] } else { self.tail[edge_idx] },
                edge_id,
            ));
        }
        self.edge_source_at(edge_idx, t).map(|&source| match source.into() {
            ShortcutSource::Shortcut(down, up) => {
                mark_upward(up);
                let lower_bound_to_middle = customized_graph.outgoing.bounds()[up as usize].0 + lower_bound_target;
                lower_bounds_to_target[customized_graph.incoming.tail[down as usize] as usize] = min(
                    lower_bounds_to_target[customized_graph.incoming.tail[down as usize] as usize],
                    lower_bound_to_middle,
                );
                customized_graph
                    .incoming
                    .evaluate_next_segment_at::<False, _>(down, t, lower_bound_to_middle, customized_graph, lower_bounds_to_target, mark_upward)
                    .unwrap()
            }
            ShortcutSource::OriginalEdge(edge) => (
                customized_graph.original_graph.travel_time_function(edge).evaluate(t),
                if Dir::VALUE { self.head[edge_idx] } else { self.tail[edge_idx] },
                edge_id,
            ),
            ShortcutSource::None => (FlWeight::INFINITY, if Dir::VALUE { self.head[edge_idx] } else { self.tail[edge_idx] }, edge_id),
        })
    }

    /// Recursively unpack the edge with the given id at the given timestamp and add the path to `result`
    pub fn unpack_at(&self, edge_id: EdgeId, t: Timestamp, customized_graph: &CustomizedGraph, result: &mut Vec<(EdgeId, Timestamp)>) {
        self.edge_source_at(edge_id as usize, t)
            .map(|&source| ShortcutSource::from(source).unpack_at(t, customized_graph, result))
            .expect("can't unpack empty shortcut");
    }

    fn edge_source_at(&self, edge_idx: usize, t: Timestamp) -> Option<&ShortcutSourceData> {
        let data = self.edge_sources(edge_idx);

        if data.is_empty() {
            return None;
        }
        if data.len() == 1 {
            return Some(&data[0].1);
        }

        let (_, t_period) = t.split_of_period();
        debug_assert!(data.first().map(|&(t, _)| t == Timestamp::zero()).unwrap_or(true), "{:?}", data);
        match data.binary_search_by_key(&t_period, |(t, _)| *t) {
            Ok(i) => data.get(i),
            Err(i) => {
                debug_assert!(data.get(i - 1).map(|&(t, _)| t < t_period).unwrap_or(true));
                if i < data.len() {
                    debug_assert!(t_period < data[i].0);
                }
                data.get(i - 1)
            }
        }
        .map(|(_, s)| s)
    }

    /// Borrow slice of all the source of the edge with given id.
    pub fn edge_sources(&self, edge_idx: usize) -> &[(Timestamp, ShortcutSourceData)] {
        &self.sources[(self.first_source[edge_idx] as usize)..(self.first_source[edge_idx + 1] as usize)]
    }
}

/// Struct with borrowed slice of the relevant parts (topology, upper and lower bounds) for elimination tree corridor query.
#[derive(Debug)]
pub struct SingleDirBoundsGraph<'a> {
    first_out: &'a [EdgeId],
    head: &'a [NodeId],
    bounds: &'a [(FlWeight, FlWeight)],
}

impl<'a> SingleDirBoundsGraph<'a> {
    pub fn num_nodes(&self) -> usize {
        self.first_out.len() - 1
    }

    fn neighbor_edge_indices_usize(&self, node: NodeId) -> Range<usize> {
        (self.first_out[node as usize] as usize)..(self.first_out[(node + 1) as usize] as usize)
    }

    pub fn neighbor_iter(&self, node: NodeId) -> impl Iterator<Item = ((NodeId, EdgeId), &(FlWeight, FlWeight))> {
        let range = self.neighbor_edge_indices_usize(node);
        let edge_ids = range.start as EdgeId..range.end as EdgeId;
        self.head[range.clone()].iter().cloned().zip(edge_ids).zip(self.bounds[range].iter())
    }
}

// Util function to skip early returns
fn always(_up: bool, _shortcut_id: EdgeId, _t: Timestamp) -> bool {
    true
}
