use std::cmp::Ordering;
use std::env;
use std::path::Path;

use bmw_routing_engine::{
    graph::{
        *,
        floating_time_dependent::*,
        time_dependent::period as int_period,
    },
    shortest_path::{
        customizable_contraction_hierarchy::{self, cch_graph::SeparatorTree},
        node_order::NodeOrder,
        query::{
            time_dependent_customizable_contraction_hierarchy::Server,
            td_dijkstra::Server as DijkServer
        },
    },
    io::Load,
    benchmark::*,
};

use time::Duration;

#[derive(PartialEq,PartialOrd)]
struct NonNan(f32);

impl NonNan {
    fn new(val: f32) -> Option<NonNan> {
        if val.is_nan() {
            None
        } else {
            Some(NonNan(val))
        }
    }
}

impl Eq for NonNan {}

impl Ord for NonNan {
    fn cmp(&self, other: &NonNan) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

fn main() {
    let mut args = env::args();
    args.next();

    let arg = &args.next().expect("No directory arg given");
    let path = Path::new(arg);

    let first_out = Vec::load_from(path.join("first_out").to_str().unwrap()).expect("could not read first_out");
    let head = Vec::load_from(path.join("head").to_str().unwrap()).expect("could not read head");
    let mut first_ipp_of_arc = Vec::load_from(path.join("first_ipp_of_arc").to_str().unwrap()).expect("could not read first_ipp_of_arc");
    let ipp_departure_time = Vec::<u32>::load_from(path.join("ipp_departure_time").to_str().unwrap()).expect("could not read ipp_departure_time");
    let ipp_travel_time = Vec::<u32>::load_from(path.join("ipp_travel_time").to_str().unwrap()).expect("could not read ipp_travel_time");

    println!("nodes: {}, arcs: {}, ipps: {}", first_out.len() - 1, head.len(), ipp_departure_time.len());

    let mut new_ipp_departure_time = Vec::with_capacity(ipp_departure_time.len() + 2 * head.len());
    let mut new_ipp_travel_time = Vec::with_capacity(ipp_departure_time.len() + 2 * head.len());

    let mut added = 0;

    for i in 0..head.len() {
        let range = first_ipp_of_arc[i] as usize .. first_ipp_of_arc[i+1] as usize;
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
            new_ipp_departure_time.push(int_period());
            new_ipp_travel_time.push(ipp_travel_time[range.start]);
            added += 1;
        } else {
            new_ipp_departure_time.push(0);
            new_ipp_travel_time.push(ipp_travel_time[range.start]);
        }
    }
    first_ipp_of_arc[head.len()] += added;

    println!("nodes: {}, arcs: {}, ipps: {}", first_out.len() - 1, head.len(), new_ipp_departure_time.len());

    let points = new_ipp_departure_time.into_iter().zip(new_ipp_travel_time.into_iter()).map(|(dt, tt)| {
        Point { at: Timestamp::new(f64::from(dt) / 1000.0), val: FlWeight::new(f64::from(tt) / 1000.0) }
    }).collect();

    let graph = TDGraph::new(first_out, head, first_ipp_of_arc, points);

    // let graph = TDGraph::new(first_out, head, first_ipp_of_arc, ipp_departure_time, ipp_travel_time);
    let cch_order = NodeOrder::from_node_order(Vec::load_from(path.join("cch_perm").to_str().unwrap()).expect("could not read cch_perm"));
    let cch = customizable_contraction_hierarchy::contract(&graph, cch_order);

    let cch_order = NodeOrder::from_node_order(Vec::load_from(path.join("cch_perm").to_str().unwrap()).expect("could not read cch_perm"));
    let latitude = Vec::<f32>::load_from(path.join("latitude").to_str().unwrap()).expect("could not read latitude");
    let longitude = Vec::<f32>::load_from(path.join("longitude").to_str().unwrap()).expect("could not read longitude");
    let cch_order = CCHReordering { node_order: cch_order, latitude, longitude }.reorder(cch.separators());
    let cch = customizable_contraction_hierarchy::contract(&graph, cch_order);

    let _td_cch_graph = cch.customize_floating_td(&graph);
    // println!("{:?}", td_cch_graph.total_num_segments());
    // td_cch_graph.print_segment_stats();

    // let mut td_dijk_server = DijkServer::new(graph.clone());
    // let mut server = Server::new(&cch, &td_cch_graph);

    // let from = Vec::load_from(path.join("uniform_queries/source_node").to_str().unwrap()).expect("could not read source node");
    // let at = Vec::load_from(path.join("uniform_queries/source_time").to_str().unwrap()).expect("could not read source time");
    // let to = Vec::load_from(path.join("uniform_queries/target_node").to_str().unwrap()).expect("could not read target node");

    // let num_queries = 100;

    // let mut dijkstra_time = Duration::zero();
    // let mut tdcch_time = Duration::zero();

    // for ((from, to), at) in from.into_iter().zip(to.into_iter()).zip(at.into_iter()).take(num_queries) {
    //     let (ground_truth, time) = measure(|| {
    //         td_dijk_server.distance(from, to, at).map(|dist| dist + at)
    //     });
    //     dijkstra_time =  dijkstra_time + time;

    //     tdcch_time = tdcch_time + measure(|| {
    //         let dist = server.distance(from, to, at).map(|dist| dist + at);
    //         if dist == ground_truth {
    //             println!("TDCCH ✅ {:?} {:?}", dist, ground_truth);
    //         } else {
    //             println!("TDCCH ❌ {:?} {:?}", dist, ground_truth);
    //         }
    //         assert_eq!(dist, ground_truth);
    //     }).1;
    // }
    // println!("Dijkstra {}ms", dijkstra_time.num_milliseconds() / (num_queries as i64));
    // println!("TDCCH {}ms", tdcch_time.num_milliseconds() / (num_queries as i64));
}

#[derive(Debug)]
struct CCHReordering {
    node_order: NodeOrder,
    latitude: Vec<f32>,
    longitude: Vec<f32>,
}

impl CCHReordering {
    fn distance (&self, n1: NodeId, n2: NodeId) -> NonNan {
        use nav_types::WGS84;
        NonNan::new(WGS84::new(self.latitude[self.node_order.node(n1) as usize], self.longitude[self.node_order.node(n1) as usize], 0.0)
            .distance(&WGS84::new(self.latitude[self.node_order.node(n2) as usize], self.longitude[self.node_order.node(n2) as usize], 0.0))).unwrap()
    }

    fn reorder_sep(&self, nodes: &mut [NodeId]) {
        if nodes.len() < 10 { return }

        let furthest = nodes.first().map(|&first| {
            nodes.iter().max_by_key(|&&node| self.distance(first, node)).unwrap()
        });

        if let Some(&furthest) = furthest {
            nodes.sort_by_key(|&node| self.distance(node, furthest))
        }
    }

    fn reorder_tree(&self, separators: &mut SeparatorTree) {
        self.reorder_sep(&mut separators.nodes);
        for child in &mut separators.children {
            self.reorder_tree(child);
            // if let Some(&first) = child.nodes.first() {
            //     if let Some(&last) = child.nodes.last() {
            //         if let Some(&node) = separators.nodes.first() {
            //             if self.distance(first, node) < self.distance(last, node) {
            //                 child.nodes.reverse()
            //             }
            //         }
            //     }
            // }
        }
    }

    fn to_ordering(&self, seperators: SeparatorTree, order: &mut Vec<NodeId>) {
        order.extend(seperators.nodes);
        for child in seperators.children {
            self.to_ordering(*child, order);
        }
    }

    fn to_ordering_bfs(&self, seperators: SeparatorTree, order: &mut Vec<NodeId>) {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(seperators);

        while let Some(seperator) = queue.pop_front() {
            order.extend(seperator.nodes);
            for child in seperator.children {
                queue.push_back(*child);
            }
        }
    }

    pub fn reorder(self, mut separators: SeparatorTree) -> NodeOrder {
        self.reorder_tree(&mut separators);
        let mut order = Vec::new();
        self.to_ordering_bfs(separators, &mut order);

        for rank in &mut order {
            *rank = self.node_order.node(*rank);
        }
        order.reverse();

        NodeOrder::from_node_order(order)
    }
}
