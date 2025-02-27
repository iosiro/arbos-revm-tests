#![no_main]

extern crate alloc;

use std::vec;

use alloy_primitives::Bytes;
use alloy_sol_types::{SolCall, SolValue};
use stylus_sdk::{
    call::RawCall, host::VM, prelude::*
};

alloy_sol_macro::sol!{
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
}


#[entrypoint]
fn user_main(input: Vec<u8>, _vm: VM) -> Result<Vec<u8>, Vec<u8>> {
    // Decode calldata
    let decoded = Multicaller::multicallCall::abi_decode(input.as_slice(), true).expect("Failed to decode calldata");

    let mut results = vec![];
    // Execute calls, for let call in decoded.calls
    for call in decoded.calls.iter() {
   

         let raw_call = match call.callType {
            Multicaller::CallType::CALL => {
                let raw_call = if call.value.is_zero() { 
                    RawCall::new()
                } else {
                    RawCall::new_with_value(call.value)
                };

                if !call.gas_limit.is_zero() {
                    raw_call.gas(call.gas_limit.as_limbs()[0])
                } else {
                    raw_call
                }               
            }
            Multicaller::CallType::DELEGATECALL => {
                let raw_call = RawCall::new_delegate();

                if !call.gas_limit.is_zero() {
                    raw_call.gas(call.gas_limit.as_limbs()[0])
                } else {
                    raw_call
                }
            }
            Multicaller::CallType::STATICCALL => {
                let raw_call = RawCall::new_static();

                if !call.gas_limit.is_zero() {
                    raw_call.gas(call.gas_limit.as_limbs()[0])
                } else {
                    raw_call
                }
            }
            Multicaller::CallType::__Invalid => todo!(),
        };

        #[allow(unused_unsafe)]
        let result = unsafe { raw_call.call(call.target, call.data.to_vec().as_slice()) };

        match result {
            Ok(result) => results.push(Bytes::from(result)),
            Err(e) => {
                return Err(e)
            }
        }
      
    }

    // Encode results
    Ok(results.abi_encode())

}