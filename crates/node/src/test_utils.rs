use std::{
    collections::{HashMap, HashSet},
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, RwLock},
    time::Duration,
};

use async_trait::async_trait;
use block::{Block, BlockHash, ClaimHash, GenesisBlock, InnerBlock, ProposalBlock};
use bulldag::{graph::BullDag, vertex::Vertex};
pub use miner::test_helpers::{create_address, create_claim, create_miner};
use primitives::{generate_account_keypair, Address, NodeId, NodeType, RawSignature, Round};
use secp256k1::{Message, PublicKey, SecretKey};
use storage::vrrbdb::Claims;
use uuid::Uuid;
use vrrb_config::{NodeConfig, NodeConfigBuilder};
use vrrb_core::{
    account::Account,
    claim::Claim,
    keypair::Keypair,
    txn::{generate_txn_digest_vec, NewTxnArgs, QuorumCertifiedTxn, TransactionDigest, Txn},
};
use vrrb_rpc::rpc::{api::RpcApiClient, client::create_client};

use crate::{
    dag_module::DagModule, data_store::DataStore, network::NetworkEvent, result::*,
    state_reader::StateReader,
};

pub fn create_mock_full_node_config() -> NodeConfig {
    let data_dir = env::temp_dir();
    let id = Uuid::new_v4().simple().to_string();

    let temp_dir_path = std::env::temp_dir();
    let db_path = temp_dir_path.join(vrrb_core::helpers::generate_random_string());

    let idx = 100;

    let http_api_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let jsonrpc_server_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let rendezvous_local_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let rendezvous_server_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let grpc_server_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 50051);
    let public_ip_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let udp_gossip_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let raptorq_gossip_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let kademlia_liveness_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);

    let default_node_config = NodeConfig::default();

    NodeConfigBuilder::default()
        .id(id)
        .idx(idx)
        .data_dir(data_dir)
        .db_path(db_path)
        .node_type(NodeType::Bootstrap)
        .bootstrap_config(None)
        .http_api_address(http_api_address)
        .http_api_title(String::from("HTTP Node API"))
        .http_api_version(String::from("1.0"))
        .http_api_shutdown_timeout(Some(Duration::from_secs(5)))
        .jsonrpc_server_address(jsonrpc_server_address)
        .keypair(Keypair::random())
        .rendezvous_local_address(rendezvous_local_address)
        .rendezvous_server_address(rendezvous_server_address)
        .udp_gossip_address(udp_gossip_address)
        .raptorq_gossip_address(raptorq_gossip_address)
        .kademlia_peer_id(None)
        .kademlia_liveness_address(default_node_config.kademlia_liveness_address)
        .public_ip_address(public_ip_address)
        .grpc_server_address(grpc_server_address)
        .disable_networking(false)
        .quorum_config(None)
        .bootstrap_quorum_config(None)
        .build()
        .unwrap()
}

#[deprecated]
pub fn create_mock_full_node_config_with_bootstrap(
    bootstrap_node_addresses: Vec<SocketAddr>,
) -> NodeConfig {
    let mut node_config = create_mock_full_node_config();

    node_config
}

#[deprecated]
pub fn create_mock_bootstrap_node_config() -> NodeConfig {
    let mut node_config = create_mock_full_node_config();

    node_config
}

pub fn produce_accounts(n: usize) -> Vec<(Address, Account)> {
    (0..n)
        .map(|_| {
            let kp = generate_account_keypair();
            let mut account = Account::new(kp.1);
            account.set_credits(1_000_000_000_000_000_000_000_000_000u128);
            (Address::new(kp.1), account)
        })
        .collect()
}

fn produce_random_claims(n: usize) -> HashSet<Claim> {
    (0..n)
        .map(|_| {
            let kp = Keypair::random();
            let address = Address::new(kp.miner_kp.1);
            let ip_address = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
            let signature = Claim::signature_for_valid_claim(
                kp.miner_kp.1,
                ip_address,
                kp.get_miner_secret_key().secret_bytes().to_vec(),
            )
            .unwrap();

            Claim::new(kp.miner_kp.1, address, ip_address, signature).unwrap()
        })
        .collect()
}

fn produce_random_txs(accounts: &Vec<(Address, Account)>) -> HashSet<Txn> {
    accounts
        .clone()
        .iter()
        .enumerate()
        .map(|(idx, (address, account))| {
            let receiver = if (idx + 1) == accounts.len() {
                accounts[0].clone()
            } else {
                accounts[idx + 1].clone()
            };

            let mut validators: Vec<(String, bool)> = vec![];

            accounts.clone().iter().for_each(|validator| {
                if (validator.clone() != receiver)
                    && (validator.clone() != (address.clone(), account.clone()))
                {
                    let pk = validator.clone().0.public_key().to_string();
                    validators.push((pk, true));
                }
            });
            create_txn_from_accounts((address.clone(), account.clone()), receiver.0, validators)
        })
        .collect()
}

pub fn produce_genesis_block() -> GenesisBlock {
    let genesis = miner::test_helpers::mine_genesis();
    genesis.unwrap()
}

pub fn produce_proposal_blocks(
    last_block_hash: BlockHash,
    accounts: Vec<(Address, Account)>,
    n: usize,
    ntx: usize,
) -> Vec<ProposalBlock> {
    (0..n)
        .map(|_| {
            let kp = Keypair::random();
            let address = Address::new(kp.miner_kp.1);
            let ip_address = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
            let signature = Claim::signature_for_valid_claim(
                kp.miner_kp.1,
                ip_address,
                kp.get_miner_secret_key().secret_bytes().to_vec(),
            )
            .unwrap();

            let from = Claim::new(kp.miner_kp.1, address, ip_address, signature).unwrap();
            let txs = produce_random_txs(&accounts);
            let claims = produce_random_claims(ntx);

            let txn_list = txs
                .into_iter()
                .map(|txn| {
                    let digest = txn.id();

                    let certified_txn = QuorumCertifiedTxn::new(
                        Vec::new(),
                        Vec::new(),
                        txn,
                        RawSignature::new(),
                        true,
                    );

                    (digest, certified_txn)
                })
                .collect();

            let claim_list = claims
                .into_iter()
                .map(|claim| (claim.hash, claim))
                .collect();

            let keypair = Keypair::random();

            ProposalBlock::build(
                last_block_hash.clone(),
                0,
                0,
                txn_list,
                claim_list,
                from,
                keypair.get_miner_secret_key(),
            )
        })
        .collect()
}

pub fn produce_convergence_block(dag: Arc<RwLock<BullDag<Block, BlockHash>>>) -> Option<BlockHash> {
    let keypair = Keypair::random();
    let mut miner = miner::test_helpers::create_miner_from_keypair(&keypair);
    miner.dag = dag.clone();
    let last_block = miner::test_helpers::get_genesis_block_from_dag(dag.clone());

    if let Some(block) = last_block {
        miner.last_block = Some(Arc::new(block));
    }

    if let Ok(cblock) = miner.try_mine() {
        if let Block::Convergence { ref block } = cblock.clone() {
            let cvtx: Vertex<Block, String> = cblock.into();
            let mut edges: Vec<(Vertex<Block, String>, Vertex<Block, String>)> = vec![];
            if let Ok(guard) = dag.read() {
                block.clone().get_ref_hashes().iter().for_each(|t| {
                    if let Some(pvtx) = guard.get_vertex(t.clone()) {
                        edges.push((pvtx.clone(), cvtx.clone()));
                    }
                });
            }

            if let Ok(mut guard) = dag.write() {
                let edges = edges
                    .iter()
                    .map(|(source, reference)| (source, reference))
                    .collect();

                guard.extend_from_edges(edges);
                return Some(block.get_hash());
            }
        }
    }

    None
}

pub fn create_keypair() -> (SecretKey, PublicKey) {
    let kp = Keypair::random();
    kp.miner_kp
}

pub fn create_txn_from_accounts(
    sender: (Address, Account),
    receiver: Address,
    validators: Vec<(String, bool)>,
) -> Txn {
    let (sk, pk) = create_keypair();
    let saddr = sender.0.clone();
    let raddr = receiver;
    let amount = 100u128.pow(2);
    let token = None;

    let validators = validators
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();

    let txn_args = NewTxnArgs {
        timestamp: 0,
        sender_address: saddr,
        sender_public_key: pk,
        receiver_address: raddr,
        token,
        amount,
        signature: sk
            .sign_ecdsa(Message::from_hashed_data::<secp256k1::hashes::sha256::Hash>(b"vrrb")),
        validators: Some(validators),
        nonce: sender.1.nonce() + 1,
    };

    let mut txn = Txn::new(txn_args);

    txn.sign(&sk);

    let txn_digest_vec = generate_txn_digest_vec(
        txn.timestamp,
        txn.sender_address.to_string(),
        txn.sender_public_key,
        txn.receiver_address.to_string(),
        txn.token.clone(),
        txn.amount,
        txn.nonce,
    );

    let _digest = TransactionDigest::from(txn_digest_vec);

    txn
}

/// Creates a `DagModule` for testing the event handler.
pub(crate) fn create_dag_module() -> DagModule {
    let miner = create_miner();
    let (sk, pk) = create_keypair();
    let addr = create_address(&pk);
    let ip_address = "127.0.0.1:8080".parse::<SocketAddr>().unwrap();
    let signature =
        Claim::signature_for_valid_claim(pk, ip_address, sk.secret_bytes().to_vec()).unwrap();

    let claim = create_claim(&pk, &addr, ip_address, signature);
    let (events_tx, _) = tokio::sync::mpsc::channel(events::DEFAULT_BUFFER);

    DagModule::new(miner.dag, events_tx, claim)
}

/// Creates a blank `block::Certificate` from a `Claim` signature.
pub(crate) fn create_blank_certificate(claim_signature: String) -> block::Certificate {
    block::Certificate {
        signature: claim_signature,
        inauguration: None,
        root_hash: "".to_string(),
        next_root_hash: "".to_string(),
        block_hash: "".to_string(),
    }
}

pub async fn create_dyswarm_client(addr: SocketAddr) -> crate::Result<dyswarm::client::Client> {
    let client_config = dyswarm::client::Config { addr };
    let client = dyswarm::client::Client::new(client_config).await?;

    Ok(client)
}

pub async fn send_data_over_quic(data: String, addr: SocketAddr) -> crate::Result<()> {
    let client = create_dyswarm_client(addr).await?;

    let msg = dyswarm::types::Message {
        id: dyswarm::types::MessageId::new_v4(),
        timestamp: 0i64,
        data: NetworkEvent::Ping(data),
    };

    client.send_data_via_quic(msg, addr).await?;

    Ok(())
}

use rand::{seq::SliceRandom, thread_rng};

pub fn generate_nodes_pattern(n: usize) -> Vec<NodeType> {
    let total_elements = 8; // Sum of occurrences: 2 + 2 + 4
    let farmer_count = n * 2 / total_elements;
    let harvester_count = n * 2 / total_elements;
    let miner_count = n * 4 / total_elements;

    let mut array = Vec::with_capacity(n);
    for _ in 0..harvester_count {
        array.push(NodeType::Validator);
    }
    for _ in 0..miner_count {
        array.push(NodeType::Miner);
    }

    array.shuffle(&mut thread_rng());

    array
}

/// Creates an instance of a RpcApiClient for testing.
pub async fn create_node_rpc_client(rpc_addr: SocketAddr) -> impl RpcApiClient {
    create_client(rpc_addr).await.unwrap()
}

/// Creates a mock `NewTxnArgs` struct meant to be used for testing.
pub fn create_mock_transaction_args(n: usize) -> NewTxnArgs {
    let (sk, pk) = create_keypair();
    let (_, rpk) = create_keypair();
    let saddr = create_address(&pk);
    let raddr = create_address(&rpk);
    let amount = (n.pow(2)) as u128;
    let token = None;

    NewTxnArgs {
        timestamp: 0,
        sender_address: saddr,
        sender_public_key: pk,
        receiver_address: raddr,
        token,
        amount,
        signature: sk
            .sign_ecdsa(Message::from_hashed_data::<secp256k1::hashes::sha256::Hash>(b"vrrb")),
        validators: None,
        nonce: n as u128,
    }
}

#[derive(Debug, Clone, Default)]
pub struct MockStateStore {}

impl MockStateStore {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug, Clone, Default)]
pub struct MockStateReader {}

impl MockStateReader {
    pub fn new() -> Self {
        MockStateReader {}
    }
}

#[async_trait]
impl StateReader for MockStateReader {
    /// Returns a full list of all accounts within state
    async fn state_snapshot(&self) -> Result<HashMap<Address, Account>> {
        todo!()
    }

    /// Returns a full list of transactions pending to be confirmed
    async fn mempool_snapshot(&self) -> Result<HashMap<TransactionDigest, Txn>> {
        todo!()
    }

    /// Get a transaction from state
    async fn get_transaction(&self, transaction_digest: TransactionDigest) -> Result<Txn> {
        todo!()
    }

    /// List a group of transactions
    async fn list_transactions(
        &self,
        digests: Vec<TransactionDigest>,
    ) -> Result<HashMap<TransactionDigest, Txn>> {
        todo!()
    }

    async fn get_account(&self, address: Address) -> Result<Account> {
        todo!()
    }

    async fn get_round(&self) -> Result<Round> {
        todo!()
    }

    async fn get_blocks(&self) -> Result<Vec<Block>> {
        todo!()
    }

    async fn get_transaction_count(&self) -> Result<usize> {
        todo!()
    }

    async fn get_claims_by_account_id(&self) -> Result<Vec<Claim>> {
        todo!()
    }

    async fn get_claim_hashes(&self) -> Result<Vec<ClaimHash>> {
        todo!()
    }

    async fn get_claims(&self, claim_hashes: Vec<ClaimHash>) -> Result<Claims> {
        todo!()
    }

    async fn get_last_block(&self) -> Result<Block> {
        todo!()
    }

    fn state_store_values(&self) -> HashMap<Address, Account> {
        todo!()
    }

    /// Returns a copy of all values stored within the state trie
    fn transaction_store_values(&self) -> HashMap<TransactionDigest, Txn> {
        todo!()
    }

    fn claim_store_values(&self) -> HashMap<NodeId, Claim> {
        todo!()
    }
}

#[async_trait]
impl DataStore<MockStateReader> for MockStateStore {
    type Error = NodeError;

    fn state_reader(&self) -> MockStateReader {
        todo!()
    }
}
