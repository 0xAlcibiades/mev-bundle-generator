// Address constants for deployed solidity contracts

// Bot Contracts
// TODO(Figure out how to include these as pre-parsed web3::types::Address/H160s)

pub(crate) const MulticallEXECUTOR: &str = "0x3312eCF4aa80937bdca0fc19E2E7De1798F8cfa7";

// Tokens

pub(crate) const ETH_ADDRESS: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";
pub(crate) const WETH_ADDRESS: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
pub(crate) const CETH_ADDRESS: &str = "0x4Ddc2D193948926D02f9B1fE9e1daa0718270ED5";
pub(crate) const SUSHI_TOKEN: &str = "0x6b3595068778dd592e39a122f4f5a5cf09c90fe2";

// Sushiswap

pub(crate) const SUSHI_MASTER_CHEF: &str = "0xc2edad668740f1aa35e4d8f227fb8e17dca888cd";
pub(crate) const SUSHI_ROUTER: &str = "0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F";

// Alpha Homora V1

pub(crate) const AH_V1_ETH_SUSHI: &str = "0x3c2bbb353b48d54b619db8ac6aa642627fb800e3";
pub(crate) const AH_V1_ETH_USDT: &str = "0xd902a3bedebad8bead116e8596497cf7d9f45da2";
pub(crate) const AH_V1_ETH_DAI: &str = "0xd6419FD982a7651A12a757Ca7cD96b969D180330";
pub(crate) const AH_V1_ETH_LINK: &str = "0xcfbd9eeac76798571ed96ed60ca34df35f29ea8d";
pub(crate) const AH_V1_ETH_USDC: &str = "0xf134fdd0bbce951e963d5bc5b0ffe445c9b6c5c6";
pub(crate) const AH_V1_ETH_WBTC: &str = "0x54a2c35d689f4314fa70dd018ea0a84c74506925";
pub(crate) const AH_V1_ETH_BAND: &str = "0xa7120893283cc2aba8155d6b9887bf228a8a86d2";
pub(crate) const AH_V1_ETH_AAVE: &str = "0xbb4755673e9df77f1af82f448d2b09f241752c05";
pub(crate) const AH_V1_ETH_COMP: &str = "0x35952c82e146da5251f2f822d7b679f34ffa71d3";
pub(crate) const AH_V1_ETH_SNX: &str = "0x8c5cecc9abd8503d167e6a7f2862874b6193e6e4";
pub(crate) const AH_V1_ETH_SUSD: &str = "0x69fe7813f804a11e2fd279eba5dc1ecf6d6bf73b";
pub(crate) const AH_V1_ETH_UMA: &str = "0x8fc4c0566606aa0c715989928c12ce254f8e1228";
pub(crate) const AH_V1_ETH_REN: &str = "0x37ef9c13faa609d5eee21f84e4c6c7bf62e4002e";
pub(crate) const AH_V1_ETH_YAM: &str = "0x9d9c28f39696ce0ebc42ababd875977060e7afa1";
pub(crate) const AH_V1_ETH_CRV: &str = "0x5c767dbf81ec894b2d70f2aa9e45a54692d0d7eb";
pub(crate) const AH_V1_ETH_YFI: &str = "0x6d0eb60d814a21e2bed483c71879777c9217aa28";
pub(crate) const AH_V1_ETH_K3PR: &str = "0x795d3655d0d7ecbf26dd33b1a7676017bb0ee611";
pub(crate) const AH_V1_ETH_BOR: &str = "0x6a279df44b5717e89b51645e287c734bd3086c1f";
pub(crate) const AH_V1_ETH_OBTC: &str = "0x1001ec1b6fc2438e8be6ffa338d3380237c0399a";

// Uniswap v2 Arbitrage

pub(crate) const UNISWAP_LOOKUP_CONTRACT_ADDRESS: &str =
    "0x5EF1009b9FCD4fec3094a5564047e190D72Bd511";
pub(crate) const UNISWAP_FACTORY_ADDRESS: &str = "0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f";
pub(crate) const SUSHISWAP_FACTORY_ADDRESS: &str = "0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac";
pub(crate) const SHIBASWAP_FACTORY_ADDRESS: &str = "0x115934131916c8b277dd010ee02de363c09d037c";
pub(crate) const CRO_FACTORY_ADDRESS: &str = "0x9DEB29c9a4c7A88a3C0257393b7f3335338D9A9D";
pub(crate) const ZEUS_FACTORY_ADDRESS: &str = "0xbdda21dd8da31d5bee0c9bb886c044ebb9b8906a";
pub(crate) const LUA_FACTORY_ADDRESS: &str = "0x0388c1e0f210abae597b7de712b9510c6c36c857";
pub(crate) const FACTORY_ADDRESSES: &[&str] = &[
    UNISWAP_FACTORY_ADDRESS,
    SUSHISWAP_FACTORY_ADDRESS,
    SHIBASWAP_FACTORY_ADDRESS,
    CRO_FACTORY_ADDRESS,
    ZEUS_FACTORY_ADDRESS,
    LUA_FACTORY_ADDRESS,
];
pub(crate) const BLACKLISTED_TOKENS: &[&str] = &[
    "0x0698dda3c390ff92722f9eed766d8b1727621df9",
    "0x9EA3b5b4EC044b70375236A281986106457b20EF",
    "0x15874d65e649880c2614e7a480cb7c9A55787FF6",
    "0xcabb170c0fabaf1cbc373f00777e46c27ba6a774",
    "0xcf8335727b776d190f9d15a54e6b9b9348439eee",
    "0x61eb53ee427ab4e007d78a9134aacb3101a2dc23",
    "0xb1e96895001281e768da8ef26232e9056f85d53d",
    "0x2d27cae0c7e88de9b85b3e44ea37b9cb70ca745f",
    "0x9f12f4b11056f4adddf08e8f56aa227010e464ac",
    "0x3312eCF4aa80937bdca0fc19E2E7De1798F8cfa7",
    "0xcd7492db29e2ab436e819b249452ee1bbdf52214",
    "0x389999216860ab8e0175387a0c90e5c52522c945",
];
pub(crate) const BLACKLISTED_POOLS: &[&str] = &[
    "0x9f12f4b11056F4addDF08e8f56AA227010E464Ac",
    "0x7A019E9f33af312b5E5c6b065fBC733CcaA09F39",
    "0x3312eCF4aa80937bdca0fc19E2E7De1798F8cfa7",
    "0x0EdEB95D2460880ed686409e88b942FD6600fF88",
    "0xcB9648D4ED92747E76DBFc5bEDeA64607970cB7a",
    "0x24b24Af104c961DA1BA5bCCce4410d49AA558477",
    "0x0bff31d8179da718a7ee3669853cf9978c90a24a",
    "0x7b890092f81b337ed68fba266afc7b4c3710a55b",
    "0xD0dCB7a4F8cFCDb29364d621Ca5D997b7EDDbc46",
    "0x7418FF4e30fBA40e43cF03999452627456eF911C",
    "0x4E9e73C0170f09e709573127c4AB02e57b868178",
    "0x459e4eEAFB9e5d7299Bbbcd5b6Ab36667FfE3597",
];
pub(crate) const ORIGIN_TOKENS: &[&str] = &[
    "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    //"0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
    //"0x6B175474E89094C44Da98b954EedeAC495271d0F",
    //"0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    //"0xdAC17F958D2ee523a2206206994597C13D831ec7",
];

// Flash Loan Providers

pub(crate) const APE_BANK: &str = "0x00000000454a11ca3a574738c0aab442b62d5d45";
