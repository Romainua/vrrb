use std::{collections::HashMap, net::SocketAddr, ops::AddAssign};

use async_trait::async_trait;
use dyswarm::{
    client::{BroadcastArgs, BroadcastConfig},
    server::ServerConfig,
};
use events::{Event, EventMessage, EventPublisher, EventSubscriber};
use kademlia_dht::{Key, Node as KademliaNode, NodeData};
use primitives::{KademliaPeerId, NodeId, NodeType};
use storage::vrrbdb::VrrbDbReadHandle;
use telemetry::info;
use theater::{Actor, ActorId, ActorImpl, ActorLabel, ActorState, Handler, TheaterError};
use tracing::Subscriber;
use utils::payload::digest_data_to_bytes;
use vrrb_config::{BootstrapQuorumConfig, NodeConfig, QuorumMembershipConfig};
use vrrb_core::claim::Claim;

use super::{NetworkEvent, NetworkModule};
use crate::{
    network::DyswarmHandler,
    result::Result,
    NodeError,
    RuntimeComponent,
    RuntimeComponentHandle,
    DEFAULT_ERASURE_COUNT,
};

#[async_trait]
impl Handler<EventMessage> for NetworkModule {
    fn id(&self) -> ActorId {
        self.id.clone()
    }

    fn label(&self) -> ActorLabel {
        format!("Network::{}", self.id())
    }

    fn status(&self) -> ActorState {
        self.status.clone()
    }

    fn set_status(&mut self, actor_status: ActorState) {
        self.status = actor_status;
    }

    async fn handle(&mut self, event: EventMessage) -> theater::Result<ActorState> {
        match event.into() {
            Event::PeerJoined(peer_data) => {
                info!("Storing peer information from {} in DHT", peer_data.node_id);

                // TODO: revisit this insert method
                self.kademlia_node.insert(
                    peer_data.kademlia_peer_id,
                    &peer_data.kademlia_liveness_addr.to_string(),
                );

                self.events_tx
                    .send(Event::NodeAddedToPeerList(peer_data.clone()).into())
                    .await
                    .map_err(|err| TheaterError::Other(err.to_string()))?;

                if let Some(quorum_config) = self.bootstrap_quorum_config.clone() {
                    let node_id = peer_data.node_id;

                    let quorum_member_ids = quorum_config
                        .membership_config
                        .quorum_members
                        .iter()
                        .cloned()
                        .map(|membership| membership.member.node_id)
                        .collect::<Vec<NodeId>>();

                    if quorum_member_ids.contains(&node_id) {
                        self.bootstrap_quorum_available_nodes.insert(node_id, true);
                    }

                    let available_nodes = self.bootstrap_quorum_available_nodes.clone();

                    if available_nodes.iter().all(|(_, is_online)| *is_online) {
                        info!("All quorum members are online. Triggering genesis quorum elections");

                        self.events_tx
                            .send(Event::GenesisQuorumMembersAvailable.into())
                            .await
                            .map_err(|err| TheaterError::Other(err.to_string()))?;
                    }
                }
            },

            Event::ClaimCreated(claim) => {
                info!("Broadcasting claim to peers");
                self.broadcast_claim(claim).await?;
            },

            Event::Stop => {
                // NOTE: stop the kademlia node instance
                self.node_ref().kill();
                return Ok(ActorState::Stopped);
            },
            Event::NoOp => {},
            _ => {},
        }

        Ok(ActorState::Running)
    }

    fn on_stop(&self) {
        info!(
            "{}-{} received stop signal. Stopping",
            self.label(),
            self.id(),
        );
    }
}