use common::{deploy_wasm, setup_simple_test, wasm_contract_init_code, DEPLOYER};
use revm::arbos::STYLUS_MAGIC_BYTES;
use revm::db::{CacheDB, EmptyDB};
use revm::primitives::bytes::Bytes;
use revm::primitives::{address, ExecutionResult, TxEnv, TxKind, B256, U256};
use alloy_sol_types::{sol, SolCall};

const CREATE_PROGRAM_BYTECODE: &[u8] = include_bytes!("assets/create_program.wasm");
const EMIT_LOG_PROGRAM_BYTECODE: &[u8] = include_bytes!("assets/emit_log.wasm");

mod common;

sol!{
    contract CreateTest {
        function create(bytes memory init_code, uint256 endowment) external returns (address);
        function create2(bytes memory init_code, bytes32 salt, uint256 endowment) external returns (address);
    }
}

fn create_test(balance: U256, endowment: U256) {
    let mut db = CacheDB::new(EmptyDB::new());

    setup_simple_test(&mut db);

    let create_address = deploy_wasm(&mut db, CREATE_PROGRAM_BYTECODE.to_vec(), DEPLOYER);

    db.load_account(create_address).unwrap().info.balance = balance;

    let code = wasm_contract_init_code(EMIT_LOG_PROGRAM_BYTECODE.to_vec());

    let expected_address = create_address.create(1);

    let calldata = CreateTest::createCall {
        init_code: code.into(),
        endowment,
    };

    {
        let evm = revm::Evm::builder()
            .with_db(&mut db)
            .modify_tx_env(|tx: &mut TxEnv| {
                tx.caller = DEPLOYER;
                tx.transact_to = TxKind::Call(create_address);
                tx.data = calldata.abi_encode().into();
                tx.gas_limit = 1e9 as u64;
            });
        let result = evm.build().transact_commit().unwrap();

        assert!(result.is_success());
    }

    let account_info = db.accounts.get(&expected_address).unwrap().info.clone();

    assert_eq!(account_info.balance, endowment);
    assert_eq!(
        account_info.code.unwrap().original_bytes().to_vec().len(),
        [
            Bytes::from(STYLUS_MAGIC_BYTES),
            Bytes::from(EMIT_LOG_PROGRAM_BYTECODE),
        ]
        .concat()
        .len()
    );
}

#[test]
pub fn create_with_no_endowment() {
    create_test(U256::ZERO, U256::ZERO);
}

#[test]
pub fn create_with_endowment() {
    create_test(U256::from(0.5e18), U256::from(0.5e18));
}

#[test]
pub fn create_with_endowment_exceeds_balance() {
    let endowment = U256::from(1.5e18);
    let mut db = CacheDB::new(EmptyDB::new());

    setup_simple_test(&mut db);

    let create_address = deploy_wasm(&mut db, CREATE_PROGRAM_BYTECODE.to_vec(), DEPLOYER);

    db.load_account(create_address).unwrap().info.balance = U256::from(0.5e18);

    let code = wasm_contract_init_code(EMIT_LOG_PROGRAM_BYTECODE.to_vec());

    let calldata = CreateTest::createCall {
        init_code: code.into(),
        endowment,
    };

    {
        let evm = revm::Evm::builder()
            .with_db(&mut db)
            .modify_tx_env(|tx: &mut TxEnv| {
                tx.caller = DEPLOYER;
                tx.transact_to = TxKind::Call(create_address);
                tx.data = calldata.abi_encode().into();
            });
        let result = evm.build().transact_commit().unwrap();

        match result {
            ExecutionResult::Revert { output, .. } => {
                assert_eq!(output, Bytes::new());
            }
            _ => panic!("Expected revert: {:?}", result),
        }
    }
}

#[test]
pub fn create_2() {
    let mut db = CacheDB::new(EmptyDB::new());

    let deployer = address!("Bd770416a3345F91E4B34576cb804a576fa48EB1");

    let deployed_address = common::deploy_wasm(&mut db, CREATE_PROGRAM_BYTECODE.to_vec(), deployer);

    let endowment = U256::from(0);
    let salt = B256::from(U256::from(1234));

    let code = wasm_contract_init_code(EMIT_LOG_PROGRAM_BYTECODE.to_vec());

    let calldata = CreateTest::create2Call {
        init_code: code.clone().into(),
        salt,
        endowment,
    };

    let expected_address = deployed_address.create2_from_code(salt, code);
    {
        let evm = revm::Evm::builder()
            .with_db(&mut db)
            .modify_tx_env(|tx: &mut TxEnv| {
                tx.caller = deployer;
                tx.transact_to = TxKind::Call(deployed_address);
                tx.data = calldata.abi_encode().into();
            });

        let result = evm.build().transact_commit().unwrap();

        assert!(result.is_success());
    }

    let account_code = db
        .accounts
        .get(&expected_address)
        .unwrap()
        .info
        .code
        .clone();
    assert_eq!(
        account_code.unwrap().original_bytes().to_vec(),
        [
            Bytes::from(STYLUS_MAGIC_BYTES),
            Bytes::from(EMIT_LOG_PROGRAM_BYTECODE),
        ]
        .concat()
    );
}
