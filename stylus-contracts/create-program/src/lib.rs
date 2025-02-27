#![cfg_attr(not(test), no_main)]
extern crate alloc;

use alloy_primitives::Address;
use stylus_sdk::{abi::Bytes, alloy_primitives::{B256, U256}, prelude::*};


#[storage]
#[entrypoint]
pub struct CreateProgram;

#[public]
impl CreateProgram {
 
    pub fn create(&mut self, init_code: Bytes, endowment: U256) -> Result<Address, Bytes> {
        match unsafe { self.vm().deploy(init_code.as_slice(), endowment, None) } {
            Ok(address) => Ok(address),
            Err(error) => Err(error.into())
        }       
    }

    pub fn create2(&mut self, init_code: Bytes, salt: B256, endowment: U256) -> Result<Address, Bytes> {
        match unsafe { self.vm().deploy(init_code.as_slice(), endowment, Some(salt)) } {
            Ok(address) => Ok(address),
            Err(error) => Err(error.into())
        }       
    }
}


