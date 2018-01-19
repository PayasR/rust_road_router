use postgres::{Connection, TlsMode, Result};
use super::*;

#[derive(Debug)]
pub struct PostgresSource {
    connection: Connection
}

impl PostgresSource {
    pub fn new(db_url: &str) -> Result<PostgresSource> {
        let connection = Connection::connect(db_url, TlsMode::None)?;
        Ok(PostgresSource { connection })
    }
}

impl RdfDataSource for PostgresSource {
    fn links(&self) -> Vec<RdfLink> {
        self.connection
            .query("select link_id, ref_node_id, nonref_node_id from rdf_link", &[])
            .unwrap()
            .into_iter()
            .map(|row| { RdfLink { link_id: row.get(0), ref_node_id: row.get(1), nonref_node_id: row.get(2) } })
            .collect()
    }

    fn nav_links(&self) -> Vec<RdfNavLink> {
        self.connection
            .query("select link_id, travel_direction, speed_category, from_ref_speed_limit, to_ref_speed_limit from rdf_nav_link", &[])
            .unwrap()
            .into_iter()
            .map(|row| { RdfNavLink { link_id: row.get(0), travel_direction: row.get::<usize, String>(1).parse().unwrap(), speed_category: row.get(2), from_ref_speed_limit: row.get(3), to_ref_speed_limit: row.get(4), } })
            .collect()
    }


    fn nodes(&self) -> Vec<RdfNode> {
        self.connection
            .query("select node_id, lat, lon, z_coord from rdf_node", &[])
            .unwrap()
            .into_iter()
            .map(|row| { RdfNode { node_id: row.get(0), lat: row.get(1), lon: row.get(2), z_coord: row.get(3) } })
            .collect()
    }

    fn link_geometries(&self) -> Vec<RdfLinkGeometry> {
        self.connection
            .query("select link_id, seq_num, lat, lon, z_coord from rdf_link_geometry", &[])
            .unwrap()
            .into_iter()
            .map(|row| { RdfLinkGeometry { link_id: row.get(0), seq_num: row.get(1), lat: row.get(2), lon: row.get(3), z_coord: row.get(4) } })
            .collect()
    }
}