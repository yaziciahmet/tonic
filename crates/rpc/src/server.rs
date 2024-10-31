use jsonrpsee::core::RpcResult;

use crate::types::SendTransactionRequest;
use crate::RpcServer;

pub struct RpcServerImpl;

#[async_trait::async_trait]
impl RpcServer for RpcServerImpl {
    async fn send_transaction(&self, _req: SendTransactionRequest) -> RpcResult<()> {
        unimplemented!("send_transaction rpc is not implemented yet")
    }
}
