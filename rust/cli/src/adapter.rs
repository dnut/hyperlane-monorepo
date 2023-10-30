use async_trait::async_trait;
use solana_client::{
    client_error::ClientError, nonblocking::rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use solana_sdk::{
    account::Account, hash::Hash, pubkey::Pubkey, signature::Signature, transaction::Transaction,
};

use crate::{AccountReader, TransactionSender};

#[async_trait(?Send)]
impl TransactionSender for (RpcClient, RpcSendTransactionConfig) {
    type Error = ClientError;

    async fn send_and_confirm_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<Signature, Self::Error> {
        self.0
            .send_and_confirm_transaction_with_spinner_and_config(
                &transaction,
                self.0.commitment(),
                self.1,
            )
            .await
    }

    async fn get_latest_blockhash(&self) -> Result<Hash, Self::Error> {
        self.0.get_latest_blockhash().await
    }
}

#[async_trait]
impl AccountReader for (RpcClient, RpcSendTransactionConfig) {
    type Error = ClientError;

    async fn get_account(&self, address: &Pubkey) -> Result<Option<Account>, Self::Error> {
        self.0
            .get_account_with_commitment(&address, self.0.commitment())
            .await
            .map(|r| r.value)
    }
}

#[cfg(test)]
mod test_util {
    use std::sync::Arc;

    use solana_banks_client::{BanksClient, BanksClientError};
    use tokio::sync::Mutex;

    use super::*;

    #[async_trait(?Send)]
    impl TransactionSender for Arc<Mutex<BanksClient>> {
        type Error = BanksClientError;

        async fn send_and_confirm_transaction(
            &self,
            transaction: Transaction,
        ) -> Result<Signature, Self::Error> {
            let ret = transaction.signatures[0];
            let simulation = self.lock().await.simulate_transaction(transaction).await?;
            if let Some(Err(err)) = simulation.result {
                return Err(BanksClientError::TransactionError(err));
            }
            Ok(ret)
        }

        async fn get_latest_blockhash(&self) -> Result<Hash, Self::Error> {
            self.lock().await.get_latest_blockhash().await
        }
    }

    #[async_trait]
    impl AccountReader for Arc<Mutex<BanksClient>> {
        type Error = BanksClientError;

        async fn get_account(&self, address: &Pubkey) -> Result<Option<Account>, Self::Error> {
            self.lock().await.get_account(*address).await
        }
    }
}
