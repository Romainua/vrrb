use node::{test_utils::create_mock_full_node_config, Node, NodeState, RuntimeModuleState, test_utils};
use primitives::{generate_account_keypair, Address};
use secp256k1::Message;
use vrrb_core::txn::NewTxnArgs;
use vrrb_rpc::rpc::{api::RpcApiClient, client::create_client};

#[tokio::test]
#[ignore = "state sync is not implemented yet"]
async fn nodes_can_synchronize_state() {
    // NOTE: two instances of a config are required because the node will create a
    // data directory for the database which cannot be the same for both nodes
    let node_config_1 = create_mock_full_node_config();
    let node_config_2 = create_mock_full_node_config();

    let start_args1 = node::StartArgs::new(
        node_config_1,
        test_utils::MockStateStore::new()
    );
    let start_args2 = node::StartArgs::new(
        node_config_2,
        test_utils::MockStateStore::new()
    );

    let vrrb_node_1 = Node::start(start_args1).await.unwrap();
    let vrrb_node_2 = Node::start(start_args2).await.unwrap();

    let client_1 = create_client(vrrb_node_1.jsonrpc_server_address())
        .await
        .unwrap();

    let client_2 = create_client(vrrb_node_2.jsonrpc_server_address())
        .await
        .unwrap();

    for _ in 0..1_00 {
        let (sk, pk) = generate_account_keypair();
        let (_, recv_pk) = generate_account_keypair();

        let signature =
            sk.sign_ecdsa(Message::from_hashed_data::<secp256k1::hashes::sha256::Hash>(b"vrrb"));
        client_1
            .create_txn(NewTxnArgs {
                timestamp: 0,
                sender_address: Address::new(pk),
                sender_public_key: pk,
                receiver_address: Address::new(recv_pk),
                token: None,
                amount: 0,
                signature,
                nonce: 0,
                validators: None,
            })
            .await
            .unwrap();
    }

    let mempool_snapshot = client_2.get_full_mempool().await.unwrap();

    assert!(!mempool_snapshot.is_empty());
    assert!(vrrb_node_1.stop().await.unwrap());
    assert!(vrrb_node_2.stop().await.unwrap());
}
