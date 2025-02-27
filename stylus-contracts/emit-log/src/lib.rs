#![cfg_attr(not(test), no_main)]
extern crate alloc;

use alloy_primitives::Address;
use stylus_sdk::{abi::Bytes, alloy_primitives::{B256, U256}, deploy::RawDeploy, evm, prelude::*};


#[storage]
#[entrypoint]
pub struct EmitLog;

#[public]
impl EmitLog {
    pub fn emitLog(&mut self, topics: Vec<B256>, data: Bytes) -> Result<(), Vec<u8>> {        
        evm::raw_log(&topics, &data).map_err(|e| e.into())
    }
}


