use std::ops::Shl;
use web3::types::{Address, U256};

// Solidity is really wasteful, and writing a general purpose contract for multicall requires
// more than a bit of thinking.
//
// Much of this is inspired by ApeBotV3, and also by hackers delight.
//
// Later it might be worth writing snappy u256 compression in rust and companion decompression in
// Yul to get rid of all the damnable 0s.

/// The type of call to execute
pub enum Type {
    Call,
    ValueCall,
    //AssertBalance,
    AssertOwnerBalance,
}

pub struct CallHeader {
    // TODO(Enum of call types to support value transfer)
    pub target: Address,
    pub method: Vec<u8>,
    pub call_type: Type,
}

pub struct Call {
    pub header: CallHeader,
    pub value: Option<U256>,
    pub payload: Vec<u8>,
}

impl Call {
    pub fn new(
        target: Address,
        method: Vec<u8>,
        call_type: Type,
        value: Option<U256>,
        payload: Vec<u8>,
    ) -> Call {
        let header = CallHeader {
            target,
            method,
            call_type,
        };
        Call {
            header,
            value,
            payload,
        }
    }
}

// The first data word is our multicall header
// The format is as follows (native endian):
// 1: bool pay_with_weth
// 2: bool burn_gastoken
// Bits 64-128: uint64 desired_block (block number)
// Bits 128-256: uint128 eth_to_coinbase (amount in wei)
pub struct MulticallHeader {
    // 256/256 bytes are in potential use.
    pub pay_with_weth: bool,
    pub burn_gastoken: bool,
    pub eth_to_coinbase: u128,
    pub desired_block: u64,
}

impl MulticallHeader {
    pub fn new(
        pay_with_weth: bool,
        burn_gastoken: bool,
        eth_to_coinbase: u128,
        desired_block: u64,
    ) -> MulticallHeader {
        MulticallHeader {
            pay_with_weth,
            burn_gastoken,
            eth_to_coinbase,
            desired_block,
        }
    }

    pub fn encode(&self) -> U256 {
        // Let's start with zero
        let mut encoded = U256::from(0);
        // Let's set the flags
        if self.pay_with_weth {
            encoded += U256::from(2);
        }
        if self.burn_gastoken {
            encoded += U256::from(4);
        }
        encoded += U256::from(self.desired_block).shl(64);
        encoded += U256::from(self.eth_to_coinbase).shl(128);
        encoded
    }
}

pub(crate) struct Multicall {
    header: MulticallHeader,
    calls: Vec<Call>,
}

impl Multicall {
    pub fn new(header: MulticallHeader, calls: Vec<Call>) -> Multicall {
        Multicall { header, calls }
    }

    // This should take the multicall header and a vector of calls
    pub fn encode_parameters(&self) -> Vec<U256> {
        let padding: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
        let mut params: Vec<U256> = vec![self.header.encode()];
        // TODO(Break this out into a function)
        for call in self.calls.iter() {
            // Build and encode call header bytes
            let mut call_header: Vec<u8> = vec![];
            call_header.extend_from_slice(&call.header.method);
            call_header.extend_from_slice(&padding);
            call_header.extend_from_slice(&call.header.target.0);

            let call_type: U256;
            match call.header.call_type {
                Type::Call => {
                    call_type = U256::from(0);
                }
                Type::ValueCall => call_type = U256::from(1).shl(198),
                //Type::AssertBalance => { call_type = U256::from(2).shl(198) }
                Type::AssertOwnerBalance => call_type = U256::from(3).shl(198),
            }
            let mut call_header_encoded = U256::from_big_endian(&call_header);
            call_header_encoded += call_type;

            // Encode call parameters
            let input_len_words = (call.payload.len()) / 32;
            let mut input_size = U256::from(input_len_words);
            input_size = input_size.shl(180);
            call_header_encoded += input_size;
            params.push(call_header_encoded);

            match call.header.call_type {
                Type::Call => {
                    let mut i = 0;
                    while i <= (call.payload.len() - 32) {
                        let n = i + 32;
                        params.push(U256::from_big_endian(&call.payload[i..n]));
                        i = n;
                    }
                }
                _ => params.push(call.value.unwrap()),
            }
        }
        params
    }
}

// Eventual Call Data Format
// uint256 txHeader
// Option<uint256 value>
// Option<bytes txData>

// txHeader: AAAAAAAA B C DD EE FF GG HHHHHH IIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII
//
// A MethodID to call
// B Differentiates call type:
//   For now: 0 = call
//            1 = assert_balance_output
//   Later:
//   Bit 1: call/delegatecall
//   Bit 2: novalue/valuetransfer_call
//   Bit 3: norevert/allowrevert
//   Bit 4: assert_balance_output
// C Pointer number 1-4 (allowing for 15 in future release) of where output for this
//   call should be saved for use in a future call.
// D Length, in words, of the output to store to pointer at C
// E Length, in words, of the input blob
// F Internal Call Code - to access private functions
// G Internal token address - to load target contract address from a stored constant.
// H Gas limit
// I Target contract address
