use web3::types::U256;

pub(crate) const ZERO_U256: U256 = U256([0_u64, 0_u64, 0_u64, 0_u64]);
pub(crate) const ONE_U256: U256 = U256([1_u64, 0_u64, 0_u64, 0_u64]);

// Ether quantities
/// 1 Ether = 1e18 Wei == 0x0de0b6b3a7640000 Wei
pub const ETHER: U256 = U256([0x0de0b6b3a7640000, 0x0, 0x0, 0x0]);
pub const FINNEY: U256 = U256([1000000000000000_u64, 0x0, 0x0, 0x0]);
