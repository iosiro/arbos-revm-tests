use alloy_sol_macro::sol;
use revm::db::{CacheDB, EmptyDB};
use revm::primitives::{address, hex, keccak256, Address, TxEnv, TxKind, U256};
use serde_json::Value;

mod common;

// Constants
const MULTICALL_BYTECODE: &[u8] = include_bytes!("assets/multicall.wasm");
const TEST_PROGRAM_BYTECODE: &[u8] = include_bytes!("assets/test_program.wasm");
const MULTICALL_EVM_BYTECODE: &str = include_str!("assets/Multicaller.bin");

sol!{
    contract Multicaller {
        enum CallType {
            CALL,
            DELEGATECALL,
            STATICCALL
        }

        struct Call {
            CallType callType;
            address target;
            bytes data;
            uint256 value;
            uint256 gas_limit;
        }

        function multicall(Call[] memory calls) external payable returns (bytes[] memory results);
    }

    contract TestProgram {
        function setStorage(bytes32 slot, bytes32 data) external;
        function getStorage(bytes32 slot) external view returns (bytes32);
    }
}

// Helper struct for test environment
struct TestSetup {
    deployer: Address,
    multicall: Address,
    storage: Address,
    multicall_evm: Address,
    db: CacheDB<EmptyDB>,
}

impl TestSetup {
    fn new() -> Self {
        let mut db = CacheDB::new(EmptyDB::new());
        let deployer = address!("Bd770416a3345F91E4B34576cb804a576fa48EB1");

        // Deploy contracts
        let multicall = common::deploy_wasm(&mut db, MULTICALL_BYTECODE.to_vec(), deployer);
        let storage = common::deploy_wasm(&mut db, TEST_PROGRAM_BYTECODE.to_vec(), deployer);

        // Initialize storage
        let slot = keccak256("some-storage-slot");
        let value = keccak256("some-storage-data");
        db.load_account(storage)
            .unwrap()
            .storage
            .insert(slot.into(), value.into());

        // Deploy EVM version
    
        let bytecode = hex::decode(MULTICALL_EVM_BYTECODE).unwrap();

        let multicall_evm = common::deploy_solidity(&mut db, bytecode, deployer);

        Self {
            deployer,
            multicall,
            storage,
            multicall_evm,
            db,
        }
    }

    fn execute(&mut self, to: Address, data: Vec<u8>) -> revm::primitives::ExecutionResult {
        let mut evm = revm::Evm::builder()
            .with_db(&mut self.db)
            .modify_tx_env(|tx: &mut TxEnv| {
                tx.caller = self.deployer;
                tx.transact_to = TxKind::Call(to);
                tx.data = data.into();
                tx.gas_limit = 1_000_000_000;
            })
            .build();

        let tx = evm.transact().unwrap();

        tx.result
    }

    fn execute_commit(&mut self, to: Address, data: Vec<u8>) -> revm::primitives::ExecutionResult {
        let mut evm = revm::Evm::builder()
            .with_db(&mut self.db)
            .modify_tx_env(|tx: &mut TxEnv| {
                tx.caller = self.deployer;
                tx.transact_to = TxKind::Call(to);
                tx.data = data.into();
                tx.gas_limit = 1_000_000_000;
            })
            .build();

        evm.transact_commit().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use alloy_sol_types::SolCall;

    use super::*;

    #[test]
    fn test_direct_storage_read() {
        let mut setup = TestSetup::new();

        let calldata = TestProgram::getStorageCall {
            slot: keccak256("some-storage-slot").into(),
        };

        let result = setup.execute(setup.storage, calldata.abi_encode().into());

        assert!(result.is_success());
        assert_eq!(
            result.output().unwrap().to_vec(),
            keccak256("some-storage-data").to_vec()
        );
    }

    #[test]
    fn test_multicall_storage_read() {
        let mut setup = TestSetup::new();

        let storage_call = TestProgram::getStorageCall {
            slot: keccak256("some-storage-slot").into(),
        };

        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::CALL,
                target: setup.storage,
                data: storage_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };

        let result = setup.execute(setup.multicall, forward_call.abi_encode().into());
        assert!(result.is_success());
       
        let result = Multicaller::multicallCall::abi_decode_returns(result.output().unwrap(), true).unwrap();
        
        assert_eq!(
            result.results[0].to_vec(),
            keccak256("some-storage-data").to_vec()
        );
    }

    #[test]
    fn test_multicall_storage_write() {
        let mut setup = TestSetup::new();

        let storage_call = TestProgram::setStorageCall {
            slot: keccak256("some-storage-slot").into(),
            data: keccak256("new-storage-value").into(),
        };
        
        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::CALL,
                target: setup.storage,
                data: storage_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };

        setup.execute_commit(setup.multicall, forward_call.abi_encode().into());

        let slot = keccak256("some-storage-slot");
        let stored_value = setup
            .db
            .load_account(setup.storage)
            .unwrap()
            .storage
            .get(&slot.into())
            .unwrap();

        assert_eq!(
            stored_value.to_be_bytes_vec(),
            keccak256("new-storage-value").to_vec()
        );
    }

    #[test]
    fn test_static_call_write_protection() {
        let mut setup = TestSetup::new();

        let storage_call = TestProgram::setStorageCall {
            slot: keccak256("some-storage-slot").into(),
            data: keccak256("new-storage-value").into(),
        };
        

        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::STATICCALL,
                target: setup.storage,
                data: storage_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };

        let result = setup.execute_commit(setup.multicall, forward_call.abi_encode().into());
        let output = String::from_utf8_lossy(result.output().unwrap());

        assert!(output.contains("WriteProtection"));
    }

    #[test]
    fn test_delegatecall_storage_context() {
        let mut setup = TestSetup::new();

        // Set up multicall contract storage
        let slot = keccak256("some-storage-slot");
        setup
            .db
            .load_account(setup.multicall)
            .unwrap()
            .storage
            .insert(slot.into(), keccak256("multicall-storage-value").into());

        let storage_call = TestProgram::getStorageCall {
            slot: keccak256("some-storage-slot").into(),
        };

        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::DELEGATECALL,
                target: setup.storage,
                data: storage_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };

        let result = setup.execute(setup.multicall, forward_call.abi_encode().into());
        assert!(result.is_success());
      
       
        let result = Multicaller::multicallCall::abi_decode_returns(result.output().unwrap(), true).unwrap();
        
        assert_eq!(
            result.results[0].to_vec(),
            keccak256("multicall-storage-value").to_vec()
        );
    }

    #[test]
    fn test_multicall_to_evm() {
        let mut setup = TestSetup::new();

        let slot = keccak256("some-storage-slot");
        let data = keccak256("new-storage-value");

        let storage_call = TestProgram::setStorageCall {
            slot,
            data,
        };

        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::CALL,
                target: setup.storage,
                data: storage_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };
        
        let result = setup.execute_commit(setup.multicall_evm, forward_call.abi_encode().into());

        assert!(result.is_success());
        let stored_value = setup.db
        .load_account(setup.storage)
        .unwrap()
        .storage
        .get(&slot.into())
        .unwrap();

        assert_eq!(
            stored_value.to_be_bytes_vec(),
            data.to_vec()
        );
    }

    #[test]
    fn test_multicall_nested_evm() {
        let mut setup = TestSetup::new();

        let slot = keccak256("some-storage-slot");
        let data = keccak256("new-storage-value");

        let get_storage_call = TestProgram::getStorageCall {
            slot,
        };

        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::CALL,
                target: setup.storage,
                data: get_storage_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };
        
        let set_storage_call = TestProgram::setStorageCall {
            slot,
            data,
        };

        let forward_call = Multicaller::multicallCall {
            calls: vec![
                Multicaller::Call {
                    callType: Multicaller::CallType::CALL,
                    target: setup.storage,
                    data: set_storage_call.abi_encode().into(),
                    value: U256::ZERO,
                    gas_limit: U256::ZERO,
                },
                Multicaller::Call {
                    callType: Multicaller::CallType::CALL,
                    target: setup.multicall_evm,
                    data: forward_call.abi_encode().into(),
                    value: U256::ZERO,
                    gas_limit: U256::ZERO,
                }
            ],
        };

        let result = setup.execute(setup.multicall, forward_call.abi_encode().into());
        assert!(result.is_success());
      
        let result = Multicaller::multicallCall::abi_decode_returns(result.output().unwrap(), true).unwrap();
        let inner_result = Multicaller::multicallCall::abi_decode_returns(&result.results[1].0, true).unwrap();
  
        assert_eq!(
            inner_result.results[0].to_vec(),
            data.to_vec()
        );
    }

    #[test]
    fn test_multicall_to_stylus() {
        let mut setup = TestSetup::new();

        let slot = keccak256("some-storage-slot");
        let data = keccak256("new-storage-value");

        let storage_call = TestProgram::setStorageCall {
            slot,
            data,
        };

        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::CALL,
                target: setup.storage,
                data: storage_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };

        let forward_call = Multicaller::multicallCall {
            calls: vec![Multicaller::Call {
                callType: Multicaller::CallType::CALL,
                target: setup.multicall,
                data: forward_call.abi_encode().into(),
                value: U256::ZERO,
                gas_limit: U256::ZERO,
            }],
        };
        
        let result = setup.execute_commit(setup.multicall_evm, forward_call.abi_encode().into());

        assert!(result.is_success());
        let stored_value = setup.db
        .load_account(setup.storage)
        .unwrap()
        .storage
        .get(&slot.into())
        .unwrap();

        assert_eq!(
            stored_value.to_be_bytes_vec(),
            data.to_vec()
        );
    }
}
