// SPDX-License-Identifier: MIT

object "Multicall" {
    code {
        // Deploy the contract
        // Store gas token burn cost in zero slot
        sstore(0, 0)
        sstore(1, 0)
        sstore(2, 0)
        datacopy(0x0, dataoffset("MulticallRuntime"), datasize("MulticallRuntime"))
        return(0x0, datasize("MulticallRuntime"))
    }
    object "MulticallRuntime" {
        /* V2 TODOs
          TODO(Consider zero decoding/encoding calldata for a gas savings)
          TODO(Basefee gas futures)
        */
        code {
            
            /* External functions */
            
            switch selector()
            
            case 0xb3a59b29 /* ostium(uint256[] calldata) external onlyOwner payable */
            {
                onlyOwner()
                // Ensure data is in the shape of a uint256[>1]
                if iszero(lt(0x64, calldatasize())) {
                    if iszero(eq(0x64, calldatasize())) {
                        revert_msg("Input must be a u256 of len >= 1")
                    }
                }
                
                // Decode input array
                let index := calldataload(0x4)
                // The length of the array is the first word after the signature
                index := add(method_offset(), index)
                let len := calldataload(index)
                // Increment the index to the first data word
                index := increment(index)
                
                // Start the multicall
                adMulticall(index, len)
            }
            
            case 0xb3ab0995 /* function largeApeCallback(address sender, uint wethToReturn, uint wbtcToReturn, uint daiToReturn, uint usdcToReturn, uint usdtToReturn, bytes calldata data) */
            {
                // Require sender is owner
                require(eq(decodeAddress(0x0), owner()), "Unauthorized sender")
                // Require caller is Ape Bank
                require(eq(caller(), ape_bank()), "Unauthorized caller")
                
                // Decode input array
                let index := 0x124
                // Here is the length of the uint256[]
                let len := calldataload(index)
                // Increment the index to the first data word of the uint256[]
                index := increment(index)
                
                // Start the multicall
                adMulticall(index, len)
                
                // Repay loan
                let ethToReturn := callvalue()
                if ethToReturn {
                    transfer_ether(ethToReturn, ape_bank())
                }
                let wethToReturn := decodeUint(0x20)
                if gt(wethToReturn, zero()) {
                    transfer_ierc20(weth(), ape_bank(), wethToReturn)
                }
                let wbtcToReturn := decodeUint(0x40)
                if gt(wbtcToReturn, zero()) {
                    transfer_ierc20(wbtc(), ape_bank(), wbtcToReturn)
                }
                let daiToReturn := decodeUint(0x60)
                if gt(daiToReturn, zero()) {
                    transfer_ierc20(dai(), ape_bank(), daiToReturn)
                }
                let usdcToReturn := decodeUint(0x80)
                if gt(usdcToReturn, zero()) {
                    transfer_ierc20(usdc(), ape_bank(), usdcToReturn)
                }
                let usdtToReturn := decodeUint(0xA0)
                if gt(usdtToReturn, zero()) {
                    transfer_ierc20(usdt(), ape_bank(), usdtToReturn)
                }
            }
            
            case 0xf3fef3a3 /* function withdraw(address token, uint256 amount) external onlyOwner */
            {
                onlyOwner()
                notPayable()
                withdraw(decodeAddress(zero()), decodeUint(word()))
            }
            
            case 0x47e7ef24 /* function deposit(address token, uint256 amount) external onlyOwner payable */
            {
                onlyOwner()
                deposit(decodeAddress(zero()), decodeUint(word()))
            }
            
            case 0x70a08231 /* function balanceOf(address token) external view returns (uint256)*/
            {
                notPayable()
                let token := decodeAddress(zero())
                let ptr, tail := balanceOf(token, address())
                return(ptr, tail)
            }
            
            case 0xd3ad716d /* function eth_to_weth(uint256 amount) external onlyOwner payable */
            {
                onlyOwner()
                deposit_to_weth(decodeUint(zero()))
            }
            
            case 0x48a81ef9 /* function weth_to_eth(uint256 amount) external onlyOwner */
            {
                onlyOwner()
                notPayable()
                withdraw_from_weth(decodeUint(zero()))
            }
            
            case 0xa0712d68 /* function mint(uint256 amount) external onlyOwner */
            {
                onlyOwner()
                notPayable()
                mint(decodeUint(zero()))
            }
            
            case 0xd8ccd0f3 /* function free(uint256 amount) external onlyOwner */
            {
                onlyOwner()
                notPayable()
                free(decodeUint(zero()))
            }
            
            
            case 0xf292f1be /* function freeCost() external*/
            {
                notPayable()
                let cost := freeCost()
                let ptr, tail := obj_allocate(word())
                mstore(ptr, cost)
                return(ptr, tail)
            }
            
            case 0xf93f20d8 /* function gasTokenBalance() external*/
            {
                notPayable()
                let bal := gasTokenBalance()
                let ptr, tail := obj_allocate(word())
                mstore(ptr, bal)
                return(ptr, tail)
            }
            
            case 0xec01e0b7 /* function updateFreeCost(uint256) external onlyOwner */
            {
                onlyOwner()
                notPayable()
                updateCost(decodeUint(zero()))
            }
            
            default { 
                // Stop on undefined method
                stop() 
            }

            /* Main loop and helpers */

            function adMulticall(index, len) {
                // Turn on the money printer
                
                let gas_start := gas()
                
                let start := index
                
                // The first data word is our multicall header
                // Parse the multi call header
                // The format is as follows (native endian):
                // 1: bool pay_with_weth
                // 2: bool burn_gastoken
                // Bits 64-128: uint64 desired_block (block number)
                // Bits 128-256: uint128 eth_to_coinbase (amount in wei)
                let multiCallHeader := calldataload(index)
                index := increment(index)
                
                if lt(calldatasize(), add(start, mul(len, word()))) {
                    revert_msg("Incorrect data size")
                }
                for
                { let end:= add(start, mul(len, word())) }
                 lt(index, end)
                { index := increment(index) }
                {
                    // TODO(Implement 8 8 byte pointer array for call return data objects)
                    let header := calldataload(index)
                    // Call Data Format
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
                    //
                    // TODO(Process call)
                    // let pointerNumber := and(shr(194, header), 0xf)
                    // let internalCall := and(shr(172, header), 0xff)
                    // let gasAllowance := and(shr(160, header), 0xffffff)
                    // I cut a bunch of these desirable features to reach a V1 of this code faster.
                    
                    index := increment(index)
                    let value := 0
                    
                    // If callType is call with input data and value
                    switch and(shr(198, header), 0xf)
                    case 0 {
                        let input_size := mul(and(shr(180, header), 0xff), word())
                        let ptr, new_index, tail := build_call(decodeMethod(header), index, input_size)
                        // Make call
                        let success := call(gas(), and(header, addressShape()), value, ptr, sub(tail, ptr), ptr, mul(and(shr(188, header), 0xff), word()))
                        if iszero(success) { revert_forward(ptr) }
                        // Let's make the index one word less to account for the end of loop increment
                        index := decrement(new_index)
                    }
                    // Value transfer call
                    case 1 {
                        // Load the value word
                        value := calldataload(index)
                        index := increment(index)
                        let input_size := mul(and(shr(180, header), 0xff), word())
                        let ptr, new_index, tail := build_call(decodeMethod(header), index, input_size)
                        // Make call
                        let success := call(gas(), and(header, addressShape()), value, ptr, sub(tail, ptr), ptr, mul(and(shr(188, header), 0xff), word()))
                        if iszero(success) { revert_forward(ptr) }
                        // Let's make the index one word less to account for the end of loop increment
                        index := decrement(new_index)
                    }
                    case 2 {
                        // callType is assert balance
                        let balance_ptr, balance_tail := balanceOf(and(header, addressShape()), address())
                        let expected := calldataload(index)
                        // Require balance greater than or equal to expected.
                        require(iszero(lt(expected, mload(balance_ptr))), "Balance too low")
                        deallocate(balance_ptr)
                    }
                    case 3 {
                        // callType is assert owner balance
                        let balance_ptr, balance_tail := balanceOf(and(header, addressShape()), owner())
                        let expected := calldataload(index)
                        // Require balance greater than or equal to expected.
                        require(iszero(lt(expected, mload(balance_ptr))), "Balance too low")
                        deallocate(balance_ptr)
                    }
                    
                }

                // Check block number
                timekeeping(multiCallHeader)
                // Burn gastoken, withdraw weth if needed, bribe coinbase if needed
                pay_miner(multiCallHeader, gas_start)
        
            }
            
            function build_call(method, index, size) -> ptr, new_index, tail {
                // Load the input data to memory
                ptr := allocate_unbounded()
                // Encode method to memory
                mstore(ptr, encodeMethod(method))
                // Set offset
                tail := add(ptr, method_offset())
                for
                { let end:= add(index, size) }
                lt(index, end)
                { index := increment(index) }
                {
                    mstore(tail, calldataload(index))
                    tail := add(tail, word())
                }
                new_index := index
            }
            
            function timekeeping(header) {
                // This is for uncle/time bandit protection
                // If the timekeeping flag (0x10) is set
                // Parse desired block number
                let desired_block := sixtyFourBitMask(shr(64, header))
                if iszero(eq(desired_block, zero())) {
                    // Require that we are in desired block
                    require(eq(number(), desired_block), "Mined in wrong block.")
                }
            }
            
            function pay_miner(header, gas_start) {
                
                // Get value of eth to send to miner
                let eth_to_coinbase := oneTwentyEightBitMask(shr(128, header))
                
                // Check if we are paying with weth
                if gt(eth_to_coinbase, zero()) {
                    if and(shr(1, header), 1) {
                        withdraw_from_weth(eth_to_coinbase)
                    }
                }
                
                // Burn gastoken if needed and get the cost
                let amount, cost := calculate_burn(header, gas_start)
                
                // Correct the miner payment for gas burn cost
                switch gt(eth_to_coinbase, cost)
                case 1 {
                    eth_to_coinbase := sub(eth_to_coinbase, cost)
                }
                case 0 {
                    eth_to_coinbase := zero()
                }
                
                // Send funds to miner, checking again to see if we need to send any.
                if gt(eth_to_coinbase, zero()) {
                    transfer_ether(coinbase(), eth_to_coinbase)
                }
                
                free(amount)
            }
            
            function calculate_burn(header, gas_start) -> amount, cost {
                // Last of all, burn tokens if required (0x4) to reduce the gas cost.
                // EIP-3529
                // V3 consideration: This will be gone because of London hard fork.
                // https://eips.ethereum.org/EIPS/eip-3198
                // Will be based on basefee and v3 could implement a native gas future.
                // TODO(Imelpment)
                cost := zero()
                if and(shr(2, header), 1) {
                    // Let's say on average, that our data is 70% zeroes, so split the diff between 16 and 4
                    let initial_gas := add(21000, mul(16, calldatasize()))
                    // Initial call cost gas, plus gas used from the start, less gas remaining, also account for sstore for free cost
                    let gas_used := add(sub(add(initial_gas, gas_start), gas()), 24205)
                    // If we are wrapped in a flash loan
                    if iszero(eq(caller(), owner())) {
                        // Then we should add about 30k gas to cover the call
                        gas_used := add(gas_used, 31000)
                    }
                    // This accounts for the burn cost per token
                    amount := div(gas_used, 24954)
                    cost := mul(amount, freeCost())
                }
            }
            
            /* Fund management */
            
            // Returns the balanceOf(token) as a memory object
            function balanceOf(token, addy) -> ptr, tail {
                // Get an unbounded pointer
                ptr := allocate_unbounded()

                switch eq(token, eth())
                case 0 // Return the IERC20 token balance of this address
                {
                    // Store the method signature for balanceOf(address)
                    mstore(ptr, encodeMethod(0x70a08231))
                    mstore(add(ptr, method_offset()), addy)
                
                    // Get balance from contract
                    let success := staticcall(gas(), token, ptr, 0x24, ptr, word())
                    if iszero(success) { revert_forward(ptr) }
                }
                case 1 // Return the eth balance of this address
                {
                    mstore(ptr, selfbalance())
                }
                
                // We are returning 32 bytes (uint256)
                tail := finalize_allocate(ptr, word())
            }
            
            // Deposits amount of token
            function deposit(token, amount){
                // TODO(Make this work)
                switch iszero(callvalue())
                case 1 // Transfer IERC20 from owner account to this address
                {
                    if eq(token, eth()) { revert_msg("Eth must be sent with callvalue") }
                    transfer_ierc20_from(token, owner(), address(), amount)
                }
                case 0 // Return the eth balance of this address
                {
                    if iszero(callvalue()) {
                    // Zero amount
                    revert_msg("Wei sent must be > 0")
                    }
                }
            }
            
            function deposit_to_weth(amount) {
                // Allocate 64 bytes
                let ptr := allocate_unbounded()
                // Store the method signature for deposit(uint256)
                mstore(ptr, encodeMethod(0xb6b55f25))
                // TODO(Set a gas limit here ?)
                let success := call(gas(), weth(), amount, ptr, method_offset(), ptr, zero())
                if iszero(success) { revert_forward(ptr) }
                // We don't need a free, because we never finalized the allocation
            }
            
            function transfer_ether(dest, amount) {
                // Could make sense, but messes with testing.
                //if iszero(dest) {
                    // Zero address
                //    revert(0, 0)
                //}
                if iszero(amount) {
                    // Zero amount
                    revert_msg("Wei amount must be > 0")
                }
                // Allocate 0 bytes
                let ptr := allocate_unbounded()
                // gas use allowed, miner address, wei to miner, zero bytes input data,
                let success := call(gas(), dest, amount, ptr, zero(), ptr, zero())
                // If the tx didn't send, we should revert.
                if iszero(success) { revert_forward(ptr) }
                // We don't need a free, because we never finalized the allocation
            }
            
            function transfer_ierc20(token, dest, amount) {
                if iszero(amount) {
                    // Zero amount
                    revert(0, 0)
                }
                let ptr := allocate_unbounded()
                // Store the method signature for transfer(address,uint256)
                mstore(ptr, encodeMethod(0xa9059cbb))
                mstore(add(ptr, method_offset()), dest)
                mstore(add(ptr, 0x24), amount)
                let success := call(gas(), token, zero(), ptr, 0x44, ptr, word())
                if iszero(success) { revert_forward(ptr) }
                require(eq(mload(ptr), 1), "ERC20 Transfer failed")
                // We never finalized the allocation, so we have nothing to free
            }
            
            function transfer_ierc20_from(token, source, dest, amount) {
                if iszero(amount) {
                    // Zero amount
                    let ptr
                    revert_msg("Amount must be >= 1 wei")
                }
                let ptr := allocate_unbounded()
                // Store the method signature for transferFrom(address,address,uint256)
                mstore(ptr, encodeMethod(0x23b872dd))
                mstore(add(ptr, method_offset()), source)
                mstore(add(ptr, 0x24), dest)
                mstore(add(ptr, 0x44), amount)
                let success := call(gas(), token, zero(), ptr, 0x64, ptr, word())
                if iszero(success) { revert_forward(ptr) }
                require(eq(mload(ptr), 1), "ERC20 TransferFrom failed")
                // We never finalized the allocation, so we have nothing to free
            }
            
            // Transfers amount of token to contract owner
            function withdraw(token, amount){
                switch eq(token, eth())
                case 0 // Send IERC20 token to ower
                {
                    transfer_ierc20(token, owner(), amount)
                }
                case 1 // Send eth to owner
                {
                    transfer_ether(owner(), amount)
                }
            }
            
            function withdraw_from_weth(amount) {
                // Allocate 64 bytes
                let ptr := allocate_unbounded()
                // Store the method signature for withdraw(uint256)
                mstore(ptr, encodeMethod(0x2e1a7d4d))
                mstore(add(ptr, method_offset()), amount)
                // TODO(Set a gas limit here ?)
                let success := call(gas(), weth(), zero(), ptr, 0x24, ptr, zero())
                if iszero(success) { revert_forward(ptr) }
                // We don't need a free, because we never finalized the allocation
            }
            
            // Mint Native Gastoken
            function mint(amount) {
                require(gt(amount, zero()), "Amount must be > 0.")
                let bal := gasTokenBalance()
                let indexSlot := add(gasTokenStartSlot(), bal)
                for
                { let endSlot:= add(indexSlot, amount) }
                 lt(indexSlot, endSlot)
                { indexSlot := add(indexSlot, 1)}
                {
                    sstore(indexSlot, 1)
                }
                // Update cost basis based on the rough cost of creation * 2
                updateCost(mul(gasprice(), 20046))
                sstore(gasTokenBalanceSlot(), add(bal, amount))
            }
            
            // Burn Chi Gastoken
            function free(amount) {
                let bal := gasTokenBalance()
                require(iszero(gt(amount, bal)), "Amount > bal.")
                let indexSlot := sub(add(gasTokenStartSlot(), bal), 1)
                for
                { let endSlot:= sub(indexSlot, amount) }
                 gt(indexSlot, endSlot)
                { indexSlot := sub(indexSlot, 1)}
                {
                    sstore(indexSlot, 0)
                }
                sstore(gasTokenBalanceSlot(), sub(bal, amount))
            }
            
            /* Global Constants 
            
               Yul does not support an idea of constants out of the box, so where constants 
               are used multiple times, I store them here as functions for global access.
            */
            
            function owner() -> o {
                // Hard coding the owner at deploy time saves 800 gas
                // per owner restricted call.
                o := 0x36273803306a3c22bc848f8db761e974697ece0d
            }
            
            // Token addresses
            
            function weth() -> w {
                w := 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2
            }
            
            function eth() -> e {
                e := 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE
            }
            
            function chi() -> c {
                c := 0x0000000000004946c0e9F43F4Dee607b0eF1fA1c
            }
            
            function wbtc() -> w {
                w := 0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599
            }
            
            function dai() -> d {
                d := 0x6B175474E89094C44Da98b954EedeAC495271d0F
            }
            
            function usdc() -> u {
                u := 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
            }
            
            function usdt() -> u {
                u := 0xdAC17F958D2ee523a2206206994597C13D831ec7
            }
            
            function ape_bank() -> ab {
                ab := 0x00000000454a11ca3a574738C0aaB442B62D5D45
            }
            
            // Constant values
            
            function zero() -> z {
                z := 0x0
            }
            
            function method_offset() -> m {
                m := 0x4
            }
            
            function word() -> w {
                w := 0x20
            }
            
            function method_shift() -> s {
                s := 0xE0
            }
            
            
            function addressShape() -> a {
                a := 0xffffffffffffffffffffffffffffffffffffffff
            }
            
            function uint64Shape() -> u {
                u := 0xffffffffffffffff
            }
            
            function uint128Shape() -> u {
                u := 0xffffffffffffffffffffffffffffffff
            }
            
            
            /* Utility */
            
            function notPayable() {
                require(eq(callvalue(), zero()), "This is not payable")
            }
            
            function onlyOwner() {
                require(eq(owner(), caller()), "Unauthorized access detected.")
            }
            
            function revert_msg(message) {
                let ptr := allocate_unbounded()
                mstore(ptr, message)
                revert(ptr, word())
            }
            
            function revert_forward(ptr) {
                // No need to free the object here, because we are stopping.
                revert(ptr, add(ptr, returndatasize()))
            }
            
            function require(condition, message) {
                if iszero(condition) { revert_msg(message) }
            }
            
            // Contract abi method selector
            function selector() -> m {
                if lt(calldatasize(), method_offset()) { 
                    // Stop if less than 4 bytes (method signature) of calldata provided
                    stop()
                }
                m := decodeMethod(calldataload(zero()))
            }
            
            function increment(location) -> new_location {
                new_location := add(location, word())
            }
            
            function decrement(location) -> new_location {
                new_location := sub(location, word())
            }
            
            
            /* Calldata Management */
            
            /* Decoders */
            
            // Decode method from word
            function decodeMethod(data) -> m {
                m := shr(method_shift(), data)
            }
            
            function decodeAddress(offset) -> v {
                v := decodeUint(offset)
                if iszero(iszero(and(v, not(addressShape())))) {
                    revert(zero(), zero())
                }
            }
            
            function sixtyFourBitMask(value) -> v {
                v := and(value, uint64Shape())
            }
            
            function oneTwentyEightBitMask(value) -> v {
                v := and(value, uint128Shape())
            }
            
            // Offset in number of bytes from start of calldata
            function decodeUint(offset) -> v {
                let pos := add(method_offset(), offset)
                if lt(calldatasize(), add(pos, word())) {
                    revert(zero(), zero())
                }
                v := calldataload(pos)
            }
            
            
            /* Encoders */
            
            // Decode method from word
            function encodeMethod(data) -> m {
                m := shl(method_shift(), data)
            }
            
            
            
            /* Memory Management 
            
            
               This is a very simple system, which really only accounts 
               for one memory object at a time.
            */
            
            // Allocate unbounded memory from the free pointer
            function allocate_unbounded() -> ptr {
                ptr := mload(zero())
                if iszero(ptr) { 
                    ptr := word()
                    mstore(zero(), ptr)
                }
            }
            
            // Finalize unbounded allocation to size
            function finalize_allocate(ptr, size) -> tail {
                tail := add(ptr, size)
                mstore(zero(), tail)
            }
            
            // Allocate a new object of size
            function obj_allocate(size) -> ptr, tail {
                // Get free pointer
                ptr := allocate_unbounded()
                tail := finalize_allocate(ptr, size)
            }

            // Store at an offset inside an object
            function obj_store(ptr, offset, value) -> tail {
                // TODO(There is a bug here)
                switch iszero(offset)
                case 0x0 {
                    ptr := add(ptr, offset)
                    mstore(ptr, value)
                    tail := add(ptr, word())
                }
                case 0x1 {
                    mstore(ptr, value)
                    tail := add(ptr, word())
                }
            }
            
            // Load contents of an object at offset
            function obj_load(ptr, offset) -> value {
                value := mload(add(ptr, offset))
            }
            
            // Deallocate an object
            function deallocate(ptr) {
                mstore(zero(), ptr)
            }
            
            /* End memory management */
            
            /* Storage Management */
            
            function gasTokenCostSlot() -> s {
                s := 0
            }
            
            function gasTokenBalanceSlot() -> t {
                t := 1
            }
            
            function gasTokenStartSlot() -> g {
                g := 2
            }
            
            function gasTokenBalance() -> b {
                b := sload(gasTokenBalanceSlot())
            }
            
            function updateCost(cost) {
                sstore(gasTokenCostSlot(), cost)
            }
            
            function freeCost() -> c {
                // Burn cost 3 times higher than purchase cost.
                c := sload(gasTokenCostSlot())
            }
            
        }
    }
}

