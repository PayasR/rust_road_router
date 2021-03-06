use super::*;
use crate::datastr::graph::time_dependent::period as int_period;
use crate::datastr::graph::Graph as GraphTrait;

type IPPIndex = u32;

/// First out based graph data structure for time-dependent graphs.
/// All data is owned.
#[derive(Debug, Clone)]
pub struct Graph {
    first_out: Vec<EdgeId>,
    head: Vec<NodeId>,
    first_ipp_of_arc: Vec<IPPIndex>,
    ipps: Vec<TTFPoint>,
}

impl Graph {
    /// Create new Graph from raw data.
    /// Performs a bit of clean up on the input.
    pub fn new(
        first_out: Vec<EdgeId>,
        head: Vec<NodeId>,
        mut first_ipp_of_arc: Vec<IPPIndex>,
        ipp_departure_time: Vec<u32>,
        ipp_travel_time: Vec<u32>,
    ) -> Graph {
        let mut new_ipp_departure_time = Vec::with_capacity(ipp_departure_time.len() + 2 * head.len());
        let mut new_ipp_travel_time = Vec::with_capacity(ipp_departure_time.len() + 2 * head.len());

        let mut added = 0;

        // Make sure all nonconst PLFs have a point at time 0 and one at time `period` and these two have the same value
        // Make sure all const PLFs have exactly one point at time 0.
        for i in 0..head.len() {
            let range = first_ipp_of_arc[i] as usize..first_ipp_of_arc[i + 1] as usize;
            assert_ne!(range.start, range.end);

            first_ipp_of_arc[i] += added;

            if range.end - range.start > 1 {
                if ipp_departure_time[range.start] != 0 {
                    new_ipp_departure_time.push(0);
                    new_ipp_travel_time.push(ipp_travel_time[range.start]);
                    added += 1;
                }
                new_ipp_departure_time.extend(ipp_departure_time[range.clone()].iter().cloned());
                new_ipp_travel_time.extend(ipp_travel_time[range.clone()].iter().cloned());
                if *new_ipp_departure_time.last().unwrap() != int_period() {
                    new_ipp_departure_time.push(int_period());
                    new_ipp_travel_time.push(ipp_travel_time[range.start]);
                    added += 1;
                }
            } else {
                new_ipp_departure_time.push(0);
                new_ipp_travel_time.push(ipp_travel_time[range.start]);
            }
        }
        first_ipp_of_arc[head.len()] += added;

        let ipps = new_ipp_departure_time
            .into_iter()
            .zip(new_ipp_travel_time.into_iter())
            .map(|(dt, tt)| TTFPoint {
                // ms to s
                at: Timestamp::new(f64::from(dt) / 1000.0),
                val: FlWeight::new(f64::from(tt) / 1000.0),
            })
            .collect();

        Graph {
            first_out,
            head,
            first_ipp_of_arc,
            ipps,
        }
    }

    /// Borrow PLF
    pub fn travel_time_function(&self, edge_id: EdgeId) -> PiecewiseLinearFunction {
        let edge_id = edge_id as usize;
        PiecewiseLinearFunction::new(&self.ipps[self.first_ipp_of_arc[edge_id] as usize..self.first_ipp_of_arc[edge_id + 1] as usize])
    }

    /// Outgoing edge iterator
    pub fn neighbor_and_edge_id_iter(&self, node: NodeId) -> impl Iterator<Item = (NodeId, EdgeId)> + '_ {
        let range = self.neighbor_edge_indices_usize(node);
        self.head[range].iter().cloned().zip(self.neighbor_edge_indices(node))
    }

    pub fn first_out(&self) -> &[EdgeId] {
        &self.first_out[..]
    }

    pub fn head(&self) -> &[NodeId] {
        &self.head[..]
    }

    /// Assert that a time annotated path is valid and that the times of the path match the edge weights at the time.
    pub fn check_path(&self, path: Vec<(NodeId, Timestamp)>) {
        let mut iter = path.into_iter();
        let mut prev = iter.next().unwrap();
        for (node, t) in iter {
            let (prev_node, prev_t) = prev;
            let edge = self.edge_index(prev_node, node).expect("path contained nonexisting edge");
            let evaled = prev_t + self.travel_time_function(edge).evaluate(prev_t);
            assert!(
                t.fuzzy_eq(evaled),
                "expected {:?} - got {:?} at edge {} from {} (at {:?}) to {}",
                evaled,
                t,
                edge,
                prev_node,
                prev_t,
                node
            );
            prev = (node, t);
        }
    }

    /// Total number of interpolation points
    pub fn num_ipps(&self) -> usize {
        self.ipps.len()
    }

    /// Number of edges with constant PLF
    pub fn num_constant(&self) -> usize {
        self.first_ipp_of_arc
            .windows(2)
            .map(|firsts| firsts[1] - firsts[0])
            .filter(|&deg| deg == 1)
            .count()
    }
}

impl GraphTrait for Graph {
    fn num_nodes(&self) -> usize {
        self.first_out.len() - 1
    }

    fn num_arcs(&self) -> usize {
        self.head.len()
    }

    fn degree(&self, node: NodeId) -> usize {
        let node = node as usize;
        (self.first_out[node + 1] - self.first_out[node]) as usize
    }
}

impl RandomLinkAccessGraph for Graph {
    fn link(&self, edge_id: EdgeId) -> Link {
        Link {
            node: self.head[edge_id as usize],
            weight: 0,
        }
    }

    fn edge_index(&self, from: NodeId, to: NodeId) -> Option<EdgeId> {
        let first_out = self.first_out[from as usize];
        let range = self.neighbor_edge_indices_usize(from);
        self.head[range].iter().position(|&head| head == to).map(|pos| pos as EdgeId + first_out)
    }

    fn neighbor_edge_indices(&self, node: NodeId) -> Range<EdgeId> {
        (self.first_out[node as usize] as EdgeId)..(self.first_out[(node + 1) as usize] as EdgeId)
    }
}

impl<'a> LinkIterable<'a, (NodeId, EdgeId)> for Graph {
    type Iter = std::iter::Zip<std::iter::Cloned<std::slice::Iter<'a, NodeId>>, std::ops::Range<EdgeId>>;
    #[inline(always)]
    fn link_iter(&'a self, node: NodeId) -> Self::Iter {
        let range = self.neighbor_edge_indices_usize(node);
        self.head[range].iter().cloned().zip(self.neighbor_edge_indices(node))
    }
}
