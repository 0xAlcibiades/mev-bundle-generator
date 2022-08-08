// SPDX-License-Identifier: UNLICENSED

pragma solidity = 0.8.6;
pragma experimental ABIEncoderV2;
import "remix_tests.sol"; // this import is automatically injected by Remix.

contract MulticallBotV2Test {
    
    uint256[] callinfo;
    
    // Flag bit indices
    // PAY_WITH_WETH = 1
    uint8 internal constant PAY_WITH_WETH = 1;
    uint8 internal constant BURN_GAS_TOKEN = 2;
    
    // Value bit indicies
    uint8 internal constant TIMEKEEPING_BIT_SHIFT = 64;
    uint8 internal constant COINBASE_BIT_SHIFT = 128;
    uint256 internal constant pay_with_weth = 1 << PAY_WITH_WETH;
    uint256 internal constant burn_gas_token = 1 << BURN_GAS_TOKEN;
    uint256 internal constant eth_to_coinbase = 1 << COINBASE_BIT_SHIFT;
    
    function testMultiCallHeader() public {
        uint256 actual_block = block.number;
        actual_block = actual_block << TIMEKEEPING_BIT_SHIFT;
        uint256 flags = 0;
        flags += pay_with_weth;
        flags += burn_gas_token;
        flags += actual_block;
        flags += eth_to_coinbase;
        callinfo.push(flags);
        // TODO(Push three call frames)
        // TODO(Push arguments for those call frames)
        Multicall(payable(0x4d470146215d085c75767b717dbB8D3b4468F893)).ostium(callinfo);
    }
}

interface Multicall {
    function balanceOf(address token) external view returns (uint256);
    function deposit(address token,uint256 amount) external;
    // TODO(Consider a bytes blob here)
    function ostium(uint256[] calldata program) external payable;
    function withdraw(address token, uint256 amount) external;
    function eth_to_weth(uint256 amount) external payable;
    function weth_to_eth(uint256 amount) external;
    function free(uint256 amount) external;
    function freeCost() external;
    function largeApeCallback(address sender, uint wethToReturn, uint wbtcToReturn, uint daiToReturn, uint usdcToReturn, uint usdtToReturn, bytes calldata data) external payable;
    function mint(uint256 amount) external;
    receive() external payable;
    fallback() external payable;
}

