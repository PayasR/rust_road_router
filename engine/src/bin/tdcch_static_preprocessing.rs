use std::{env, error::Error, path::Path};

#[macro_use]
extern crate bmw_routing_engine;
use bmw_routing_engine::{
    cli::CliErr,
    datastr::{graph::*, node_order::NodeOrder},
    io::*,
    report::*,
    shortest_path::customizable_contraction_hierarchy::*,
};

fn main() -> Result<(), Box<dyn Error>> {
    let _reporter = enable_reporting();

    report!("program", "tdcch");
    report!("start_time", format!("{}", time::now_utc().rfc822()));
    report!("args", env::args().collect::<Vec<String>>());

    let mut args = env::args();
    args.next();

    let arg = &args.next().ok_or(CliErr("No directory arg given"))?;
    let path = Path::new(arg);

    let first_out = Vec::load_from(path.join("first_out"))?;
    let head = Vec::load_from(path.join("head"))?;
    let weight = vec![0; head.len()];

    let graph = OwnedGraph::new(first_out, head, weight);

    report!("graph", { "num_nodes": graph.num_nodes(), "num_arcs": graph.num_arcs() });

    let mut algo_runs_ctxt = push_collection_context("algo_runs".to_string());

    let cch_folder = path.join("cch");

    let cch_order = NodeOrder::from_node_order(Vec::load_from(path.join("cch_perm"))?);
    let cch_build_ctxt = algo_runs_ctxt.push_collection_item();
    let cch = contract(&graph, cch_order.clone());
    drop(cch_build_ctxt);

    let latitude = Vec::<f32>::load_from(path.join("latitude"))?;
    let longitude = Vec::<f32>::load_from(path.join("longitude"))?;

    let cch_order = CCHReordering {
        node_order: cch_order,
        latitude: &latitude,
        longitude: &longitude,
    }
    .reorder(cch.separators());

    let cch_build_ctxt = algo_runs_ctxt.push_collection_item();
    let cch = contract(&graph, cch_order.clone());
    drop(cch_build_ctxt);

    let cch_order = CCHReordering {
        node_order: cch_order,
        latitude: &latitude,
        longitude: &longitude,
    }
    .reorder_for_seperator_based_customization(cch.separators());
    if !cch_folder.exists() {
        std::fs::create_dir(&cch_folder)?;
    }
    cch_order.deconstruct_to(&cch_folder)?;

    let cch_build_ctxt = algo_runs_ctxt.push_collection_item();
    let cch = contract(&graph, cch_order);
    drop(cch_build_ctxt);

    cch.deconstruct_to(&cch_folder)?;

    Ok(())
}
