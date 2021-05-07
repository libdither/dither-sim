use plotters::prelude::*;
use plotters::coord::types::RangedCoordf32;
//use plotters::style::RGBColor;

use nalgebra::Point2;
use petgraph::{Graph, data::{Element, FromElements}};

use std::{collections::HashMap, ops::Range};

use crate::{NetSim, Node, internet::NetAddr};

const DEFAULT_BACKGROUND: RGBColor = RGBColor(200, 200, 200);

pub trait GraphPlottable {
	fn gen_graph(&self) -> Graph<(String, Point2<i32>), RGBColor>;
}

pub fn default_graph<GI: GraphPlottable>(item: &GI, render_range: &(Range<i32>, Range<i32>), image_output: &str, image_dimensions: (u32,u32)) -> anyhow::Result<()> {
	let graph_data = item.gen_graph();
	let root = BitMapBackend::new(image_output, image_dimensions).into_drawing_area();
	
	let to_tuple = |point: Point2<f32>| {
		(point[0], point[1])
	};

	// Set background color
	root.fill(&DEFAULT_BACKGROUND)?;
	// Make sure it uses correct graph layout with 4 quadrants
	let eb = 20.; // Edge buffer
	let logic_x = -(render_range.0.end as f32 + eb)..(render_range.0.end as f32 + eb);
	let logic_y = (render_range.1.end as f32 + eb)..-(render_range.1.end as f32 + eb);
	let root = root.apply_coord_spec(Cartesian2d::<RangedCoordf32, RangedCoordf32>::new(
		logic_x,
		logic_y,
		(0..image_dimensions.0 as i32, 0..image_dimensions.1 as i32),
	));

	// Draw Connections
	use petgraph::visit::EdgeRef;
	for node_idx in graph_data.node_indices() {
		let node_coord = &graph_data[node_idx].1.clone().map(|n|n as f32);
		for edge in graph_data.edges_directed(node_idx, petgraph::EdgeDirection::Outgoing) {
			let remote_idx = edge.target();
			let remote_coord = graph_data[remote_idx].1.clone().map(|n|n as f32);
	
			// offset connections so both directions show side by side
			let offset = (nalgebra::Rotation2::new(std::f32::consts::FRAC_PI_2) * (node_coord - remote_coord)).normalize();
			let offset_node_coord = node_coord + (offset * 1.);
			let offset_remote_coord = remote_coord + (offset * 1.);
			// Draw offset edge with passed color
			root.draw(&PathElement::new([to_tuple(offset_node_coord), to_tuple(offset_remote_coord)], ShapeStyle::from(edge.weight()).stroke_width(3)))?;
		}
	}
	
	// Draw Nodes
	use plotters::style::text_anchor::{Pos, HPos, VPos};
	for node in graph_data.raw_nodes() {
		let (label, position) = &node.weight;
		let position = (position[0] as f32, position[1] as f32);
		
		root.draw(&(EmptyElement::at(position) // Outer object
			+ Circle::new((0, 0), 20, ShapeStyle::from(&BLACK).filled()) // Draw Circle
			+ Text::new( // Draw Text
				label.clone(),
				(0, 0),
				("sans-serif", 30.0).into_font().color(&WHITE).pos(Pos::new(HPos::Center, VPos::Center)),
			))
		)?;
	}
	Ok(())
}

#[cfg(feature="plot")]
impl GraphPlottable for Node {
	fn gen_graph(&self) -> Graph<(String, Point2<i32>), RGBColor> {
		/* for _node in self.route_map.nodes() {


		} */
		/* let node_index self.direct_sorted.iter().map(|(_, id)|self.remotes[id].)
		self.route_map.clone().into_graph().filter_map(|idx, node_id|{
			let remote = self.remote(&id).ok();
			remote.map(|r|r.route_coord.map(|c|(id, c)))
		}, |idx, _|{

		}) */
		Graph::with_capacity(0, 0)
	}
}

impl GraphPlottable for NetSim<Node> {
	fn gen_graph(&self) -> Graph<(String, Point2<i32>), RGBColor> {
		//let root = BitMapBackend::new(path, dimensions).into_drawing_area();
		/* for (idx, node) in &self.nodes {

		} */
		let nodes: Vec<Element<(String, Point2<i32>),RGBColor>> = self.router.node_map.iter().map(|(&net_addr, lc)|{
			Element::Node {
				weight: (
					net_addr.to_string(),
					lc.position.map(|i|i as i32),
				)
			}
		}).collect();

		let node_idx_map = &self.router.node_map.iter().enumerate().map(|(idx,(&id,_))|(id,idx)).collect::<HashMap<NetAddr,usize>>();

		let edges = self.nodes.iter().enumerate().map(|(_, (net_addr, node))|{
			node.remotes.iter().filter_map(move |(_,remote)|{
				// Set color based on 
				remote.session().ok().map(|s|{
					s.direct().ok().map(|d|{
						let color = if s.is_peer() { RGBColor(0,0,0) } else { RGBColor(255,255,255) };
						(d.net_addr, color)
					})
				}).flatten()
			}).map(move |(remote_net_addr, color)|{
				Element::Edge {
					source: node_idx_map[&net_addr],
					target: node_idx_map[&remote_net_addr],
					weight: color,
				}
			})
		}).flatten();
		let graph = Graph::from_elements(nodes.into_iter().chain(edges));
		graph
	}
}
