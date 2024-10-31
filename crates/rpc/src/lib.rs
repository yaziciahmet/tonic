#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;
pub mod types;

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
pub use types::*;

#[rpc(client, server)]
pub trait Rpc {
    #[method(name = "tonic_sendTransaction")]
    async fn send_transaction(&self, req: SendTransactionRequest) -> RpcResult<()>;
}
