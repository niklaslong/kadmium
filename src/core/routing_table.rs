use std::collections::{HashMap, HashSet};

use crate::core::id::Id;

/// The core routing table data structure.
pub(crate) struct RoutingTable<T, U> {
    // The node's local identifier.
    pub(crate) local_id: Id,
    // The maximum number of identifiers that can be contained in a bucket.
    pub(crate) max_bucket_size: u8,
    // The number of addresses to share when responding to a FIND_K_NODES query.
    pub(crate) k: u8,
    // The buckets constructed for broadcast purposes.
    pub(crate) buckets: HashMap<u32, HashSet<Id>>,
    // Maps identifiers to peer meta data (connected and disconnected).
    pub(crate) peer_list: HashMap<Id, U>,
    // Maps connection identifiers to peer identifiers (connected only).
    pub(crate) id_list: HashMap<T, Id>,
}
