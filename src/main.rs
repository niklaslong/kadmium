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

fn main() {}

// #![feature(int_log)]
//
// use std::{collections::HashMap, net::SocketAddr};
//
// use kadcast::tree::RoutingTable;
// use pea2pea::{
//     protocols::{Reading, Writing},
//     Node as PNode, Pea2Pea,
// };
// use rand::{seq::SliceRandom, thread_rng};
// use time::OffsetDateTime;
//
// fn main() {
//     let a: u128 = 3;
//     let b: u128 = 2;
//
//     println!("{}", a ^ b);
//     // println!("{}", (a ^ b).log2());
// }
//
// // TODO: Nonce and Id should be at least 160 bits.
// type Nonce = u128;
// type Id = u128;
//
// const K: u8 = 20;
// const A: u8 = 3;
//
// struct PingPayload {
//     nonce: Nonce,
//     // The sending node's ID.
//     id: Id,
//     // The sending node's addr.
//     addr: SocketAddr,
// }
//
// struct PongPayload {
//     // The nonce sent in the corresponding Ping.
//     nonce: Nonce,
//     // The sending node's ID.
//     id: Id,
// }
//
// struct FindNodePayload {
//     nonce: Nonce,
//     id: Id,
// }
//
// struct ChunkPayload {
//     nonce: Nonce,
//     h: u8,
// }
//
// enum Message {
//     // Node liveness.
//     Ping(PingPayload),
//     Pong(PongPayload),
//     // Node lookup.
//     FindNode(FindNodePayload),
//     // Propagation.
//     Chunk(ChunkPayload),
// }
//
// struct Node {
//     pnode: PNode,
//     routing_table: RoutingTable,
// }

// impl Node {
//     fn process_ping(&mut self, payload: PingPayload) {
//         // Insert the peer's routing information into the table.
//         self.routing_table.insert(payload.id, payload.addr);
//
//         // TODO: respond with PONG.
//     }
//
//     fn process_pong(&mut self, payload: PongPayload) {
//         // Update the bucket order, the index of the bucket in the list is the XOR distance.
//         self.routing_table
//             .update_last_seen(payload.id, OffsetDateTime::now_utc())
//     }
//
//     fn process_find_node(&self, payload: FindNodePayload) {
//         self.routing_table.find_k_closest(payload.id);
//
//         // TODO: respond with KNODES.
//     }
//
//     fn process_k_nodes() {}
//
//     fn process_chunk(&self, payload: ChunkPayload) {
//         // This is where the buckets come in handy. When a node processes a chunk message, it
//         // selects peers in buckets ]h, 0] and propagates the CHUNK message. If h = 0, no
//         // propagation occurs.
//
//         let mut rng = thread_rng();
//
//         // TODO: verify received data, if it is malicious don't propagate further to avoid DoS.
//
//         if payload.h == 0 {
//             return;
//         }
//
//         let mut next_addrs = vec![];
//         for h in 0..payload.h {
//             // Select a peer at random in each bucket.
//             // TODO: prioritise connected peers, make a connection if absolutely necessary but this
//             // makes the broadcast less reliable.
//             if let Some(next_id) = self.routing_table.buckets[h as usize].choose(&mut rng) {
//                 next_addrs.push(self.routing_table.peer_meta.get(next_id));
//             }
//         }
//
//         // TODO: propagate the CHUNK message to each next_addr.
//     }
// }

// impl Pea2Pea for Node {
//     fn node(&self) -> &PNode {
//         &self.pnode
//     }
// }

// impl Reading for Node {}
//
// impl Writing for Node {}
