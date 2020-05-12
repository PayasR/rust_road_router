use super::*;
use crate::{
    algo::customizable_contraction_hierarchy::{query::stepped_elimination_tree::SteppedEliminationTree, *},
    datastr::{node_order::*, timestamped_vector::TimestampedVector},
    util::in_range_option::InRangeOption,
};

pub mod query;
pub mod td_query;

pub trait Potential {
    fn init(&mut self, target: NodeId);
    fn potential(&mut self, node: NodeId) -> Option<Weight>;
    fn num_pot_evals(&self) -> usize;
}

#[derive(Debug)]
pub struct CCHPotential<'a> {
    cch: &'a CCH,
    stack: Vec<NodeId>,
    potentials: TimestampedVector<InRangeOption<Weight>>,
    forward_cch_graph: FirstOutGraph<&'a [EdgeId], &'a [NodeId], Vec<Weight>>,
    backward_elimination_tree: SteppedEliminationTree<'a, FirstOutGraph<&'a [EdgeId], &'a [NodeId], Vec<Weight>>>,
    num_pot_evals: usize,
}

impl<'a> CCHPotential<'a> {
    pub fn new<Graph>(cch: &'a CCH, lower_bound: &Graph) -> Self
    where
        Graph: for<'b> LinkIterGraph<'b> + RandomLinkAccessGraph + Sync,
    {
        let customized = customize(cch, lower_bound);
        let (forward_up_graph, backward_up_graph) = customized.into_ch_graphs();
        let backward_elimination_tree = SteppedEliminationTree::new(backward_up_graph, cch.elimination_tree());

        Self {
            cch,
            stack: Vec::new(),
            forward_cch_graph: forward_up_graph,
            backward_elimination_tree,
            potentials: TimestampedVector::new(cch.num_nodes(), InRangeOption::new(None)),
            num_pot_evals: 0,
        }
    }
}

impl<'a> Potential for CCHPotential<'a> {
    fn init(&mut self, target: NodeId) {
        self.potentials.reset();
        self.backward_elimination_tree.initialize_query(self.cch.node_order().rank(target));
        while self.backward_elimination_tree.next().is_some() {
            self.backward_elimination_tree.next_step();
        }
        self.num_pot_evals = 0;
    }

    fn potential(&mut self, node: NodeId) -> Option<u32> {
        let node = self.cch.node_order().rank(node);
        self.num_pot_evals += 1;

        let mut cur_node = node;
        while self.potentials[cur_node as usize].value().is_none() {
            self.stack.push(cur_node);
            if let Some(parent) = self.backward_elimination_tree.parent(cur_node).value() {
                cur_node = parent;
            } else {
                break;
            }
        }

        while let Some(node) = self.stack.pop() {
            let min_by_up = self
                .forward_cch_graph
                .neighbor_iter(node)
                .map(|edge| edge.weight + self.potentials[edge.node as usize].value().unwrap())
                .min()
                .unwrap_or(INFINITY);

            self.potentials[node as usize] = InRangeOption::new(Some(std::cmp::min(self.backward_elimination_tree.tentative_distance(node), min_by_up)));
        }

        let dist = self.potentials[node as usize].value().unwrap();
        if dist < INFINITY {
            Some(dist)
        } else {
            None
        }
    }

    fn num_pot_evals(&self) -> usize {
        self.num_pot_evals
    }
}

#[derive(Debug)]
pub struct CHPotential {
    order: NodeOrder,
    potentials: TimestampedVector<InRangeOption<Weight>>,
    forward: OwnedGraph,
    backward_dijkstra: SteppedDijkstra<OwnedGraph>,
    num_pot_evals: usize,
}

impl CHPotential {
    pub fn new(forward: OwnedGraph, backward: OwnedGraph, order: NodeOrder) -> Self {
        let n = forward.num_nodes();
        Self {
            order,
            potentials: TimestampedVector::new(n, InRangeOption::new(None)),
            forward,
            backward_dijkstra: SteppedDijkstra::new(backward),
            num_pot_evals: 0,
        }
    }

    fn potential_internal(
        potentials: &mut TimestampedVector<InRangeOption<Weight>>,
        forward: &OwnedGraph,
        backward: &SteppedDijkstra<OwnedGraph>,
        node: NodeId,
    ) -> Weight {
        if let Some(pot) = potentials[node as usize].value() {
            return pot;
        }

        let min_by_up = forward
            .neighbor_iter(node)
            .map(|edge| edge.weight + Self::potential_internal(potentials, forward, backward, edge.node))
            .min()
            .unwrap_or(INFINITY);

        potentials[node as usize] = InRangeOption::new(Some(std::cmp::min(backward.tentative_distance(node), min_by_up)));

        potentials[node as usize].value().unwrap()
    }
}

impl Potential for CHPotential {
    fn init(&mut self, target: NodeId) {
        self.num_pot_evals = 0;
        self.potentials.reset();
        self.backward_dijkstra.initialize_query(Query {
            from: self.order.rank(target),
            to: std::u32::MAX,
        });
        while let QueryProgress::Settled(_) = self.backward_dijkstra.next_step() {}
    }

    fn potential(&mut self, node: NodeId) -> Option<Weight> {
        let node = self.order.rank(node);
        self.num_pot_evals += 1;

        let dist = Self::potential_internal(&mut self.potentials, &self.forward, &self.backward_dijkstra, node);

        if dist < INFINITY {
            Some(dist)
        } else {
            None
        }
    }

    fn num_pot_evals(&self) -> usize {
        self.num_pot_evals
    }
}

#[derive(Debug)]
pub struct TurnExpandedPotential<Potential> {
    potential: Potential,
    tail: Vec<NodeId>,
}

impl<P> TurnExpandedPotential<P> {
    pub fn new(graph: &dyn Graph, potential: P) -> Self {
        let mut tail = Vec::with_capacity(graph.num_arcs());
        for node in 0..graph.num_nodes() {
            for _ in 0..graph.degree(node as NodeId) {
                tail.push(node as NodeId);
            }
        }

        Self { potential, tail }
    }
}

impl<P: Potential> Potential for TurnExpandedPotential<P> {
    fn init(&mut self, target: NodeId) {
        self.potential.init(self.tail[target as usize])
    }
    fn potential(&mut self, node: NodeId) -> Option<Weight> {
        self.potential.potential(self.tail[node as usize])
    }
    fn num_pot_evals(&self) -> usize {
        self.potential.num_pot_evals()
    }
}
