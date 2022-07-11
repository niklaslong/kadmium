use std::{cmp::Ordering, collections::HashMap, net::SocketAddr};

use time::OffsetDateTime;

const K: u8 = 20;

type Id = u128;

#[derive(Debug, Clone, Copy)]
enum ConnState {
    Connected,
    Disconneted,
}

#[derive(Debug, Clone, Copy)]
pub struct PeerMeta {
    pub listening_addr: SocketAddr,
    pub last_seen: OffsetDateTime,
    pub conn_state: ConnState,
}

impl PeerMeta {
    fn new(listening_addr: SocketAddr, last_seen: OffsetDateTime, conn_state: ConnState) -> Self {
        Self {
            listening_addr,
            last_seen,
            conn_state,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum EntryState {
    Exists,
    Inserted,
    Pending,
    SelfEntry,
}

pub struct RoutingTable {
    pub local_id: Id,
    pub buckets: Vec<Vec<Id>>,
    // We could make the cache larger than 1 item per bucket in future, e.g. with a circular queue.
    pub pending: HashMap<u32, Id>,
    pub peer_meta: HashMap<Id, PeerMeta>,
}

impl Default for RoutingTable {
    fn default() -> Self {
        Self {
            // TODO: generate a random u128.
            local_id: 0u128,
            buckets: vec![vec![]; 128],
            pending: HashMap::new(),
            peer_meta: HashMap::new(),
        }
    }
}

impl RoutingTable {
    // TODO: perhaps we need an enum return type with Inserted, Pending, and SelfEntry.
    pub fn insert(&mut self, id: Id, peer_meta: PeerMeta) -> EntryState {
        // Buckets should only contain connected peers. The other structures should track
        // connection state.

        // Insert can happen in two instances:
        //
        // 1. the peer initiated the connection (should only be inserted if there is space in the
        //    bucket it would be in).
        // 2. the peer was included in a list from another peer (should be inserted as
        //    disconnected unless it is already in the list and is connected).
        //
        // Solution: insert all addresses as disconnected initially, returning whether the relevant
        // bucket would have space. The caller can then use this information to determine whether
        // to initiate a connection in case 1, or accept the connection in case 2.

        // Calculate the distance by XORing the ids.
        let distance = id ^ self.local_id;

        // Don't calculate the log if distance is 0, this should only happen if the ID we got from
        // the peer is the same as ours.
        if distance == u128::MIN {
            return EntryState::SelfEntry;
        }

        // Calculate the index of the bucket from the distance.
        // Nightly feature.
        let i = distance.log2();

        let bucket = self
            .buckets
            .get_mut(i as usize)
            .expect("bucket should exist");

        // This is O(n), perhaps check the hashmap instead?
        // Q: should an entry in the hashmap be removed when a node is removed from the bucket?
        if bucket.contains(&id) {
            return EntryState::Exists;
        }

        match bucket.len().cmp(&K.into()) {
            Ordering::Less => {
                // Bucket still has space.
                bucket.push(id);
                self.peer_meta
                    .insert(id, PeerMeta::new(addr, OffsetDateTime::now_utc()));

                EntryState::Inserted
            }
            Ordering::Equal => {
                // Bucket is full.
                self.pending.insert(i, id);

                EntryState::Pending
            }
            Ordering::Greater => {
                // Bucket is over capacity, this should never happen.
                // TODO: consider using an array instead of a vec?
                unreachable!()
            }
        }
    }

    pub fn sort(&mut self) {
        // Sort the buckets by last seen, this should be called every time we measure the latency
        // for a peer (PING messages).
        self.buckets.iter_mut().for_each(|bucket| {
            bucket.sort_by(|a, b| {
                let a_ls = self.peer_meta.get(a).unwrap().last_seen;
                let b_ls = self.peer_meta.get(b).unwrap().last_seen;

                // We want the most recently seen nodes at the head of the Vec.
                b_ls.partial_cmp(&a_ls).unwrap()
            })
        });
    }

    pub fn update_last_seen(&mut self, id: Id, last_seen: OffsetDateTime) {
        if let Some(peer_meta) = self.peer_meta.get_mut(&id) {
            peer_meta.last_seen = last_seen
        }
    }

    pub fn find_k_closest(&self, id: Id) -> Vec<(Id, PeerMeta)> {
        // Find the K closest nodes to the given ID. There is a total order over the keyspace, so a
        // sort won't yield any conflicts.
        //
        // Naive way: just iterate over all the IDs and XOR them? Need a map of ID to the addr, to
        // be sent to the requesting node.
        let mut ids: Vec<_> = self
            .peer_meta
            .iter()
            .map(|(&candidate_id, &candidate_meta)| (candidate_id, candidate_meta))
            .collect();
        ids.sort_by_key(|(candidate_id, _)| candidate_id ^ id);
        ids.truncate(K.into());

        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_insert() {
        let mut rt = RoutingTable::default();

        // Attempt to insert our local id.
        assert_eq!(
            rt.insert(rt.local_id, "127.0.0.1:1".parse().unwrap()),
            EntryState::SelfEntry
        );
        // Attempt to insert the next id (in base 10).
        // TODO: should fail when inserting the same addr.
        assert_eq!(
            rt.insert(rt.local_id + 1, "127.0.0.1:1".parse().unwrap()),
            EntryState::Inserted
        );
        // Attempt to insert the same id again.
        assert_eq!(
            rt.insert(rt.local_id + 1, "127.0.0.1:1".parse().unwrap()),
            EntryState::Exists
        );
    }

    #[test]
    fn last_seen_update() {
        let mut rt = RoutingTable::default();
        let peer_id = rt.local_id + 1;

        // Insert a peer ID.
        assert_eq!(
            rt.insert(peer_id, "127.0.0.1:1".parse().unwrap()),
            EntryState::Inserted
        );
        let initial_last_seen = rt.peer_meta.get(&peer_id).unwrap().last_seen;
        let expected_final_last_seen = OffsetDateTime::now_utc();

        // Update the timestamp.
        rt.update_last_seen(peer_id, expected_final_last_seen);

        let final_last_seen = rt.peer_meta.get(&peer_id).unwrap().last_seen;

        // Check the timestamp has been updated.
        assert_eq!(final_last_seen, expected_final_last_seen);
        assert!(final_last_seen > initial_last_seen);
    }

    #[test]
    fn bucket_sort() {
        let mut rt = RoutingTable::default();

        // ... 0001 -> bucket i = 0
        rt.insert(1, "127.0.0.1:1".parse().unwrap());
        // ... 0010 -> bucket i = 1
        rt.insert(2, "127.0.0.1:2".parse().unwrap());
        // ... 0011 -> bucket i = 1
        rt.insert(3, "127.0.0.1:3".parse().unwrap());
        // ... 0010 -> bucket i = 2
        rt.insert(4, "127.0.0.1:4".parse().unwrap());
        // ... 0011 -> bucket i = 2
        rt.insert(5, "127.0.0.1:5".parse().unwrap());

        assert_eq!(rt.buckets[0], vec![1]);
        assert_eq!(rt.buckets[1], vec![2, 3]);
        assert_eq!(rt.buckets[2], vec![4, 5]);

        rt.sort();

        assert_eq!(rt.buckets[0], vec![1]);
        assert_eq!(rt.buckets[1], vec![3, 2]);
        assert_eq!(rt.buckets[2], vec![5, 4]);

        // TODO: consider calling sort when updating last seen?
        rt.update_last_seen(2, OffsetDateTime::now_utc());

        assert_eq!(rt.buckets[0], vec![1]);
        assert_eq!(rt.buckets[1], vec![3, 2]);
        assert_eq!(rt.buckets[2], vec![5, 4]);

        rt.sort();

        assert_eq!(rt.buckets[0], vec![1]);
        assert_eq!(rt.buckets[1], vec![2, 3]);
        assert_eq!(rt.buckets[2], vec![5, 4]);
    }

    #[test]
    fn eviction() {
        let mut rt = RoutingTable::default();
    }
}
