use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use crate::edge::Edge;
use crate::node::{Item, Node};

/// In-memory typed knowledge graph: nodes keyed by id, with directed edges indexed
/// on both endpoints.
#[derive(Debug, Default)]
pub struct Graph {
    nodes: BTreeMap<String, Node>,
    edges: Vec<Edge>,
    outgoing: BTreeMap<String, Vec<usize>>,
    incoming: BTreeMap<String, Vec<usize>>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a node, keeping the first on an id collision.
    pub fn insert(&mut self, node: Node) -> bool {
        match self.nodes.entry(node.id()) {
            Entry::Vacant(slot) => {
                slot.insert(node);
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    /// Add a directed edge and index it on both endpoints.
    pub fn link(&mut self, edge: Edge) {
        let idx = self.edges.len();
        self.outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(idx);
        self.incoming.entry(edge.to.clone()).or_default().push(idx);
        self.edges.push(edge);
    }

    pub fn get(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// A node to change in place. Only Studio uses this, to show a decision before the next
    /// full assembly; a build never edits what it has inserted.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    pub fn has(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Nodes in id order.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values()
    }

    /// Nodes to change in place. Only Studio uses this; a build never edits what it inserted.
    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.nodes.values_mut()
    }

    /// Item nodes in id order.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.nodes.values().filter_map(|n| match n {
            Node::Item(i) => Some(i),
            _ => None,
        })
    }

    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.iter()
    }

    /// Edges leaving a node.
    pub fn from(&self, id: &str) -> Vec<&Edge> {
        self.pick(self.outgoing.get(id))
    }

    /// Edges arriving at a node.
    pub fn into(&self, id: &str) -> Vec<&Edge> {
        self.pick(self.incoming.get(id))
    }

    fn pick(&self, idx: Option<&Vec<usize>>) -> Vec<&Edge> {
        idx.map(|v| v.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }
}
