use node::{test_utils::{create_mock_bootstrap_node_config, create_mock_full_node_config_with_bootstrap}, Node, NodeState, RuntimeModuleState, test_utils};
use primitives::node::NodeType;
use serial_test::serial;
use vrrb_rpc::rpc::{api::RpcApiClient, client::create_client};

#[tokio::test]
#[serial]
async fn node_can_start_as_a_bootstrap_node() {
    let node_config = create_mock_bootstrap_node_config();

    let start_args = node::StartArgs::new(
        node_config,
        test_utils::MockStateStore::new()
    );

    let mut vrrb_node = Node::start(start_args).await.unwrap();

    let client = create_client(vrrb_node.jsonrpc_server_address())
        .await
        .unwrap();

    assert_eq!(client.get_node_type().await.unwrap(), NodeType::Bootstrap);

    assert!(vrrb_node.is_bootstrap());
    let is_cancelled = vrrb_node.stop().await.unwrap();

    assert!(is_cancelled);
}

#[tokio::test]
#[serial]
#[ignore]
async fn node_can_join_network() {
    let node_config = create_mock_bootstrap_node_config();

    let start_args = node::StartArgs::new(
        node_config,
        test_utils::MockStateStore::new()
    );

    let mut bootstrap_node = Node::start(start_args).await.unwrap();

    // NOTE: use quic for peer discovery
    let bootstrap_gossip_address = bootstrap_node.udp_gossip_address();

    let node_config_1 = create_mock_full_node_config_with_bootstrap(vec![bootstrap_gossip_address]);

    let start_args1 = node::StartArgs::new(
        node_config_1,
        test_utils::MockStateStore::new()
    );

    let mut node_1 = Node::start(start_args1).await.unwrap();
    let addr = node_1.jsonrpc_server_address();

    let client = create_client(addr).await.unwrap();

    assert!(client.is_connected());
    let state = client.get_full_state().await;

    assert!(state.is_ok());
    assert_eq!(client.get_node_type().await.unwrap(), NodeType::Bootstrap);

    let is_cancelled = node_1.stop().await.unwrap();

    assert!(is_cancelled);

    bootstrap_node.stop();
}

#[tokio::test]
#[serial]
async fn bootstrap_node_can_add_newly_joined_peers_to_peer_list() {
    let node_config = create_mock_bootstrap_node_config();

    let start_args = node::StartArgs::new(
        node_config,
        test_utils::MockStateStore::new()
    );

    let mut vrrb_node = Node::start(start_args).await.unwrap();

    let client = create_client(vrrb_node.jsonrpc_server_address())
        .await
        .unwrap();

    assert!(vrrb_node.is_bootstrap());
    assert_eq!(client.get_node_type().await.unwrap(), NodeType::Bootstrap);

    let is_cancelled = vrrb_node.stop().await.unwrap();
    assert!(is_cancelled);
}
