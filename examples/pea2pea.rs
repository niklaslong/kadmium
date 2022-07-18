// Messages:
//
// - PING: transmits the node's routing information to the recipient.
// - FIND_NODE:
//
// Data structures:
//
// - Tree
// - Bucket
//
// Global parameters:
//
// - α: number of nodes to query for k closest nodes (distance is d(x, y) = XOR(x, y) in base 10),
// in practice α = 3.
// - k: bucket size (upper bound?), in practice k in [20, 100]
//
// Lookup procedure:
//
// 1. The node looks up the α closest nodes with respect to the XOR-metric in its own buckets.
// 2. It queries these α nodes for the ID by sending FIND_NODE messages.
// 3. The queried nodes respond with a set of k nodes they believe to be closest to ID.
// 4. Based on the acquired information, the node builds a new set of closest nodes and iteratively
//    repeats steps (1)–(3), until an iteration does not yield any nodes closer than the already
//    known ones anymore.

use kadcast::tree::RoutingTable;
use pea2pea::{Node as PNode, Pea2Pea};

fn main() {}

struct Node {
    pnode: PNode,
    routing_table: RoutingTable,
}

impl Node {}

impl Pea2Pea for Node {
    fn node(&self) -> &PNode {
        &self.pnode
    }
}

// impl Reading for Node {}
//
// impl Writing for Node {}
