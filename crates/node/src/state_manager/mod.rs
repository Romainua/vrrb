mod component;
mod state_handler;
mod state_manager;

pub use component::*;
pub use state_handler::*;
pub use state_manager::*;

#[cfg(test)]
mod tests {
    use std::{
        env,
        sync::{Arc, RwLock},
    };

    use block::{Block, BlockHash};
    use bulldag::{graph::BullDag, vertex::Vertex};
    use events::{Event, DEFAULT_BUFFER};
    use mempool::LeftRightMempool;
    use primitives::Address;
    use serial_test::serial;
    use storage::vrrbdb::{VrrbDb, VrrbDbConfig};
    use theater::{Actor, ActorImpl, ActorState};
    use tokio::sync::mpsc::channel;
    use vrrb_core::{account::Account, txn::Txn};

    use super::*;
    use crate::test_utils::{
        produce_accounts,
        produce_convergence_block,
        produce_genesis_block,
        produce_proposal_blocks,
    };

    #[tokio::test]
    #[serial]
    async fn state_runtime_module_starts_and_stops() {
        let _temp_dir_path = env::temp_dir().join("state.json");

        let (events_tx, _) = tokio::sync::mpsc::channel(DEFAULT_BUFFER);

        let db_config = VrrbDbConfig::default();

        let dag: Arc<RwLock<BullDag<Block, String>>> = Arc::new(RwLock::new(BullDag::new()));

        let db = VrrbDb::new(db_config);
        let mempool = LeftRightMempool::new();

        let state_module = StateManager::new(StateManagerConfig {
            events_tx,
            mempool,
            database: db,
            dag: dag.clone(),
        });

        let mut state_module = ActorImpl::new(state_module);

        let (ctrl_tx, mut ctrl_rx) = tokio::sync::broadcast::channel(DEFAULT_BUFFER);

        assert_eq!(state_module.status(), ActorState::Stopped);

        let handle = tokio::spawn(async move {
            state_module.start(&mut ctrl_rx).await.unwrap();
            assert_eq!(state_module.status(), ActorState::Terminating);
        });

        ctrl_tx.send(Event::Stop.into()).unwrap();

        handle.await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn state_runtime_receives_new_txn_event() {
        let _temp_dir_path = env::temp_dir().join("state.json");

        let (events_tx, _) = tokio::sync::mpsc::channel(DEFAULT_BUFFER);
        let db_config = VrrbDbConfig::default();

        let db = VrrbDb::new(db_config);
        let mempool = LeftRightMempool::default();

        let dag: Arc<RwLock<BullDag<Block, String>>> = Arc::new(RwLock::new(BullDag::new()));

        let state_module = StateManager::new(StateManagerConfig {
            events_tx,
            mempool,
            database: db,
            dag: dag.clone(),
        });

        let mut state_module = ActorImpl::new(state_module);

        let (ctrl_tx, mut ctrl_rx) = tokio::sync::broadcast::channel(DEFAULT_BUFFER);

        assert_eq!(state_module.status(), ActorState::Stopped);

        let handle = tokio::spawn(async move {
            state_module.start(&mut ctrl_rx).await.unwrap();
        });

        ctrl_tx
            .send(Event::NewTxnCreated(Txn::null_txn()).into())
            .unwrap();

        ctrl_tx.send(Event::Stop.into()).unwrap();

        handle.await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn state_runtime_can_publish_events() {
        let _temp_dir_path = env::temp_dir().join("state.json");

        let (events_tx, mut events_rx) = tokio::sync::mpsc::channel(DEFAULT_BUFFER);

        let db_config = VrrbDbConfig::default();

        let db = VrrbDb::new(db_config);
        let mempool = LeftRightMempool::default();

        let dag: StateDag = Arc::new(RwLock::new(BullDag::new()));

        let state_module = StateManager::new(StateManagerConfig {
            mempool,
            events_tx,
            database: db,
            dag: dag.clone(),
        });

        let mut state_module = ActorImpl::new(state_module);

        let events_handle = tokio::spawn(async move {
            let _res = events_rx.recv().await;
        });

        let (ctrl_tx, mut ctrl_rx) = tokio::sync::broadcast::channel(DEFAULT_BUFFER);

        assert_eq!(state_module.status(), ActorState::Stopped);

        let handle = tokio::spawn(async move {
            state_module.start(&mut ctrl_rx).await.unwrap();
        });

        // TODO: implement all state && validation ops

        ctrl_tx
            .send(Event::NewTxnCreated(Txn::null_txn()).into())
            .unwrap();

        ctrl_tx.send(Event::Stop.into()).unwrap();

        handle.await.unwrap();
        events_handle.await.unwrap();
    }

    pub type StateDag = Arc<RwLock<BullDag<Block, BlockHash>>>;

    #[ignore = "state write is not yet persistent in the state module"]
    #[tokio::test]
    async fn vrrbdb_should_update_with_new_block() {
        let path = std::env::temp_dir().join("db");
        let db_config = VrrbDbConfig::default().with_path(path);
        let db = VrrbDb::new(db_config);
        let mempool = LeftRightMempool::default();
        let accounts: Vec<(Address, Account)> = produce_accounts(5);
        let dag: StateDag = Arc::new(RwLock::new(BullDag::new()));
        let (events_tx, _) = channel(100);
        let config = StateManagerConfig {
            mempool,
            database: db,
            events_tx,
            dag: dag.clone(),
        };

        let mut state_module = StateManager::new(config);
        let state_res = state_module.extend_accounts(accounts.clone());
        let genesis = produce_genesis_block();

        assert!(state_res.is_ok());

        let gblock: Block = genesis.clone().into();
        let gvtx: Vertex<Block, BlockHash> = gblock.into();
        if let Ok(mut guard) = dag.write() {
            guard.add_vertex(&gvtx);
        }

        let proposals = produce_proposal_blocks(genesis.hash, accounts.clone(), 5, 5);

        let edges: Vec<(Vertex<Block, BlockHash>, Vertex<Block, BlockHash>)> = {
            proposals
                .into_iter()
                .map(|pblock| {
                    let pblock: Block = pblock.into();
                    let pvtx: Vertex<Block, BlockHash> = pblock.into();
                    (gvtx.clone(), pvtx)
                })
                .collect()
        };

        if let Ok(mut guard) = dag.write() {
            edges
                .iter()
                .for_each(|(source, reference)| guard.add_edge((source, reference)));
        }

        let block_hash = produce_convergence_block(dag).unwrap();
        state_module.update_state(block_hash).unwrap();

        state_module.commit();

        let handle = state_module.read_handle();
        let store = handle.state_store_values();

        for (address, _) in accounts.iter() {
            let account = store.get(address).unwrap();
            let digests = account.digests().clone();
            dbg!(&digests);
            assert!(digests.get_sent().len() > 0);
            assert!(digests.get_recv().len() > 0);
            assert!(digests.get_stake().len() == 0);
        }
    }
}
