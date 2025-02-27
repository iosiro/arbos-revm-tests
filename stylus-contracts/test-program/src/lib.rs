#![cfg_attr(not(test), no_main)]
extern crate alloc;

use alloy_primitives::Address;
use stylus_sdk::{abi::Bytes, alloy_primitives::{B256, U256}, prelude::*};


#[storage]
#[entrypoint]
pub struct TestProgram;

#[public]
impl TestProgram {
    pub fn returnData(data: Bytes) -> Bytes {
        data
    }

    pub fn setStorage(&mut self, key: B256, value: B256) {
        unsafe { self.vm().storage_cache_bytes32(key.into(), value) };
    }

    pub fn getStorage(&mut self, key: B256) -> B256 {
        self.vm().storage_load_bytes32(key.into()) 
    }

    pub fn accountBalance(&mut self, account: Address) -> U256 {
        self.vm().balance(account) 
    }

    pub fn accountCode(&mut self, account: Address) -> Bytes {
        self.vm().code(account).into()
    }
}


