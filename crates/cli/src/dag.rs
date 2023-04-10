use ahash::HashMap;

use ahash::HashMapExt;
use anyhow::bail;
use anyhow::Error;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Node {
    id: Uuid,
    edges: Vec<Uuid>,
}

impl Node {
    pub fn new(id: Uuid, edges: Vec<Uuid>) -> Self {
        Self { id, edges }
    }
}

pub fn invert_graph(graph: &[Node]) -> Vec<Node> {
    let mut inverted_nodes: HashMap<Uuid, Node> = graph
        .iter()
        .map(|node| (node.id, Node::new(node.id, vec![])))
        .collect();

    for node in graph {
        for edge_id in &node.edges {
            if let Some(inverted_node) = inverted_nodes.get_mut(edge_id) {
                inverted_node.edges.push(node.id);
            }
        }
    }

    inverted_nodes.into_iter().map(|(_, node)| node).collect()
}

pub fn topological_sort(graph: &[Node]) -> Result<Vec<Vec<Uuid>>, Error> {
    let mut in_degree = HashMap::new();
    let mut execution_graph = Vec::new();
    let mut queue = Vec::new();
    let graph_map: HashMap<Uuid, &Node> = graph.iter().map(|node| (node.id, node)).collect();

    for node in graph {
        in_degree.insert(node.id, 0);
    }

    for node in graph {
        for edge in &node.edges {
            *in_degree.get_mut(edge).unwrap() += 1;
        }
    }

    for node in graph {
        if in_degree[&node.id] == 0 {
            queue.push(node.id);
        }
    }

    while !queue.is_empty() {
        let mut current = Vec::new();
        let size = queue.len();

        for _ in 0..size {
            let node = queue.pop().unwrap();
            current.push(node.clone());

            if let Some(n) = graph_map.get(&node) {
                for adjacent in &n.edges {
                    *in_degree.get_mut(adjacent).unwrap() -= 1;

                    if in_degree[adjacent] == 0 {
                        queue.push(adjacent.clone());
                    }
                }
            }
        }

        execution_graph.push(current);
    }

    if graph.iter().any(|node| in_degree[&node.id] != 0) {
        bail!("cyclical job dependencies detected");
    }

    Ok(execution_graph)
}
