#[derive(Clone, Debug)]
#[repr(u8)]
pub enum ChainId {
    Testnet = 200,
    Mainnet = 201,
}
