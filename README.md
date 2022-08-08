# Bundle Generator

This flashbots bundle generator is a graph based flashbots bundle generator intended 
to be used to hook up various MEV opportunies, traverse a graph, and construct a 
profitable flashbots bundle for each block.

A previous iteration of this was winning many of the uniswap v2 top of block 
arbitrage opportunities for the months preceding EIP-1559.

This bundle generator is implemented in Rust, with a custom Yul multicall.