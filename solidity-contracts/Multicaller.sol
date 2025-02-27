// SPDX-License-Identifier: MIT
pragma solidity ^0.8.12;

// solc Multicaller.sol --bin

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

    function multicall(Call[] memory calls) external payable returns (bytes[] memory results) {
        results = new bytes[](calls.length);
        for (uint256 i = 0; i < calls.length; i++) {
            Call memory call = calls[i];
            bool success;
            bytes memory data; 
            if (call.callType == CallType.CALL) {
                if (call.gas_limit == 0) 
                    (success, data) = call.target.call{value: call.value}(call.data);
                else 
                    (success, data) = call.target.call{value: call.value, gas: call.gas_limit}(call.data);
            } else if (call.callType == CallType.DELEGATECALL) {
                if (call.gas_limit == 0) 
                    (success, data) = call.target.delegatecall(call.data);
                else 
                    (success, data) = call.target.delegatecall{gas: call.gas_limit}(call.data);
            } else if (call.callType == CallType.STATICCALL) {
                if (call.gas_limit == 0) 
                    (success, data) = call.target.staticcall(call.data);
                else 
                    (success, data) = call.target.staticcall{gas: call.gas_limit}(call.data);
            }
            
            // bubble up revert
            if (!success) {
                assembly {
                    revert(add(data, 0x20), mload(data))
                }
            }
            results[i] = data;
        }
    }
}