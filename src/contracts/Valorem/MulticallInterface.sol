// SPDX-License-Identifier: MIT

pragma solidity = 0.8.6;
pragma experimental ABIEncoderV2;

interface Multicall {
    function balanceOf(address token) external view returns (uint256);
    function deposit(address token,uint256 amount) external;
    // TODO(Consider a bytes blob here)
    function ostium(uint256[] calldata program) external payable;
    function withdraw(address token, uint256 amount) external;
    function eth_to_weth(uint256 amount) external payable;
    function weth_to_eth(uint256 amount) external;
    function free(uint256 amount) external;
    function freeCost() external view returns (uint256);
    function updateFreeCost(uint256) external;
    function largeApeCallback(address sender, uint wethToReturn, uint wbtcToReturn, uint daiToReturn, uint usdcToReturn, uint usdtToReturn, bytes calldata data) external payable;
    function mint(uint256 amount) external;
    receive() external payable;
    fallback() external payable;
}
