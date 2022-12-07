use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use sha256::digest;
use udp2p::node::peer_id::PeerId;

use crate::message::{AsMessage, Message};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StateBlock(pub u128);

/// Message types are the different types of messages that can be
/// packed and sent across the network.
//TODO: Convert Vec<u8>, String, u128 and other standard types with custom types
// that better describe their purpose
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageType {
    TxnMessage {
        txn: Vec<u8>,
        sender_id: String,
    },
    TxnValidatorMessage {
        txn_validator: Vec<u8>,
        sender_id: String,
    },
    BlockMessage {
        block: Vec<u8>,
        sender_id: String,
    },
    ClaimMessage {
        claim: Vec<u8>,
        sender_id: String,
    },
    GetNetworkStateMessage {
        sender_id: String,
        requested_from: String,
        requestor_address: SocketAddr,
        requestor_node_type: Vec<u8>,
        lowest_block: u128,
        component: Vec<u8>,
    },
    InvalidBlockMessage {
        block_height: u128,
        reason: Vec<u8>,
        miner_id: String,
        sender_id: String,
    },
    DisconnectMessage {
        sender_id: String,
        pubkey: String,
    },
    StateComponentsMessage {
        data: Vec<u8>,
        requestor: String,
        requestor_id: String,
        sender_id: String,
    },
    GenesisMessage {
        data: Vec<u8>,
        requestor: String,
        requestor_id: String,
        sender_id: String,
    },
    ChildMessage {
        data: Vec<u8>,
        requestor: String,
        requestor_id: String,
        sender_id: String,
    },
    ParentMessage {
        data: Vec<u8>,
        requestor: String,
        requestor_id: String,
        sender_id: String,
    },
    LedgerMessage {
        data: Vec<u8>,
        requestor: String,
        requestor_id: String,
        sender_id: String,
    },
    NetworkStateMessage {
        data: Vec<u8>,
        requestor: String,
        requestor_id: String,
        sender_id: String,
    },
    ClaimAbandonedMessage {
        claim: Vec<u8>,
        sender_id: String,
    },
    DKGPartCommitmentMessage {
        dkg_part_commitment: Vec<u8>,
        sender_id: String,
    },
    DKGACKCommitmentMessage {
        dkg_ack_commitment: Vec<u8>,
        sender_id: String,
    },
    SendPeerIDMessage {
        pub_key: String,
        peer_id: PeerId,
    },
    ResetPeerConnectionMessage {
        peer_id: PeerId,
    },
    RemovePeerMessage {
        peer_id: PeerId,
        socket_addr: SocketAddr,
    },
    AddPeerMessage {
        peer_id: PeerId,
        socket_addr: SocketAddr,
    },
    SendChainLockSignatureMessage {
        chain_lock_signature: Vec<u8>,
    },
}

impl MessageType {
    /// Serialize a message to into a vector of bytes
    pub fn as_bytes(self) -> Vec<u8> {
        serde_json::to_string(&self).unwrap().as_bytes().to_vec()
    }

    /// Deserialize a vector of bytes into a MessageType
    pub fn from_bytes(data: &[u8]) -> Option<MessageType> {
        if let Ok(message) = serde_json::from_slice::<MessageType>(data) {
            Some(message)
        } else {
            None
        }
    }
}

impl StateBlock {
    pub fn as_bytes(&self) -> Vec<u8> {
        serde_json::to_string(self).unwrap().as_bytes().to_vec()
    }
}

impl AsMessage for MessageType {
    fn into_message(self, return_receipt: u8) -> Message {
        Message {
            id: digest(&*self.clone().as_bytes()).as_bytes().to_vec(),
            source: None,
            data: self.as_bytes(),
            sequence_number: None,
            signature: None,
            topics: None,
            key: None,
            validated: 0,
            return_receipt,
        }
    }
}