use std::net::SocketAddr;

use events::AssignedQuorumMembership;
use mempool::TxnRecord;
use primitives::{KademliaPeerId, NodeId, NodeType, PeerId};
use serde::{Deserialize, Serialize};
use vrrb_config::QuorumMembershipConfig;
use vrrb_core::claim::Claim;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
/// Represents data trasmitted over the VRRB network by nodes that participate
/// in it
pub enum NetworkEvent {
    ClaimCreated {
        node_id: NodeId,
        claim: Claim,
    },

    ClaimAbandoned {
        claim: Vec<u8>,
        sender_id: String,
    },

    PeerJoined {
        node_id: NodeId,
        node_type: NodeType,
        kademlia_peer_id: KademliaPeerId,
        udp_gossip_addr: SocketAddr,
        raptorq_gossip_addr: SocketAddr,
        kademlia_liveness_addr: SocketAddr,
    },

    /// Peer was assigned to a specific quorum by a bootstrap node
    AssignmentToQuorumCreated {
        assigned_membership: AssignedQuorumMembership,
    },

    AssignmentToQuorumReceived {
        assigned_membership: AssignedQuorumMembership,
    },

    /// Peer is unresponsive or signaled its intent to leave the network
    PeerUnregistered {
        peer_id: PeerId,
        socket_addr: SocketAddr,
    },

    ForwardedTxn(TxnRecord),

    Ping(NodeId),

    #[default]
    Empty,
}
