use std::io::Cursor;

use async_trait::async_trait;
use borsh::BorshDeserialize;
use hyperlane_core::Decode;
use hyperlane_sealevel_mailbox::spl_noop;
use instruction::MailboxAccounts;
use solana_sdk::{
    account::Account, hash::Hash, instruction::Instruction, message::Message, pubkey::Pubkey,
    signature::Signature, signer::Signer, signers::Signers, transaction::Transaction,
};

/// Basic constructors for relevant instructions.
pub mod instruction;

/// Implementations for dependencies of this service.
pub mod adapter;

/// Ability to engage in all interactions with an RPC node that are necessary to
/// submit a transaction to a Solana cluster.
#[async_trait(?Send)]
pub trait TransactionSender {
    type Error: std::fmt::Debug;

    async fn send_and_confirm_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<Signature, Self::Error>;

    async fn get_latest_blockhash(&self) -> Result<Hash, Self::Error>;

    /// Create and send a transaction
    async fn send_and_confirm_as_transaction(
        &self,
        instructions: &[Instruction],
        payer: &Pubkey,
        signers: impl Signers,
    ) -> Result<Signature, Self::Error> {
        let recent_blockhash = self.get_latest_blockhash().await?;
        let message = Message::new(instructions, Some(payer));
        let mut transaction = Transaction::new_unsigned(message);
        transaction.try_sign(&signers, recent_blockhash).unwrap(); // TODO

        self.send_and_confirm_transaction(transaction).await
    }
}

/// Ability to read account state from a Solana cluster.
#[async_trait]
pub trait AccountReader {
    type Error: std::fmt::Debug;

    async fn get_account(&self, address: &Pubkey) -> Result<Option<Account>, Self::Error>;

    async fn get_account_deserialized<T: BorshDeserialize>(
        &self,
        address: &Pubkey,
    ) -> Result<Option<T>, Self::Error> {
        match self.get_account(&address).await? {
            Some(account) => {
                let deserialized = T::deserialize(&mut &account.data.as_slice()[1..]).unwrap(); // TODO
                Ok(Some(deserialized))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;
    use std::sync::{Arc, RwLock};

    use hyperlane_core::{config, TxnReceiptInfo, H256};
    use hyperlane_core::{Decode, HyperlaneMessage};
    use hyperlane_sealevel_mailbox::accounts::DispatchedMessage;
    use hyperlane_sealevel_mailbox::{instruction::OutboxDispatch, spl_noop};
    use hyperlane_test_utils::mailbox_id;
    use solana_banks_client::BanksClient;
    use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
    use solana_program_test::{processor, ProgramTest};
    use solana_sdk::{
        commitment_config::{CommitmentConfig, CommitmentLevel},
        signature::Keypair,
    };
    use tokio::sync::Mutex;

    use crate::instruction as ix;

    use super::*;

    #[tokio::test]
    async fn dispatch_and_relay_from_payer_to_noop_via_1_mailbox_to_itself() {
        let (client, payer) = test_client().await;

        let (instruction, mailbox) =
            instruction::initialize_mailbox(&mailbox_id(), payer.pubkey(), 0, spl_noop::ID);
        client
            .send_and_confirm_as_transaction(&[instruction], &payer.pubkey(), [&payer])
            .await
            .unwrap();

        let recipient_program_id = spl_noop::id();
        let recipient_hash = H256(recipient_program_id.to_bytes());

        let (instruction, message_signer, message_address) = ix::outbox_dispatch(
            &mailbox_id(),
            &mailbox.outbox,
            &payer.pubkey(),
            OutboxDispatch {
                sender: payer.pubkey(),
                destination_domain: 0,
                recipient: recipient_hash,
                message_body: vec![3; 2],
            },
        );
        client
            .send_and_confirm_as_transaction(
                &[instruction],
                &payer.pubkey(),
                [&payer, &message_signer],
            )
            .await
            .unwrap();

        let message = client
            .get_account_deserialized::<DispatchedMessage>(&message_address)
            .await
            .unwrap()
            .unwrap();

        let mut reader = Cursor::new(message.encoded_message);
        let message = HyperlaneMessage::read_from(&mut reader).unwrap();

        let instruction = ix::inbox_process(
            &mailbox_id(),
            &mailbox.inbox,
            &payer.pubkey(),
            vec![],
            &message,
            ix::empty(recipient_program_id),
            ix::empty(spl_noop::ID),
            ix::empty(recipient_program_id),
        );

        client
            .send_and_confirm_as_transaction(&[instruction], &payer.pubkey(), [&payer])
            .await
            .unwrap();
    }

    async fn test_client() -> (impl TransactionSender + AccountReader, Keypair) {
        local_validator_client().await
        // program_test_client().await
    }

    async fn program_test_client() -> (Arc<Mutex<BanksClient>>, Keypair) {
        let mut program_test = ProgramTest::new(
            "hyperlane_sealevel_mailbox",
            mailbox_id(),
            processor!(hyperlane_sealevel_mailbox::processor::process_instruction),
        );

        program_test.add_program("spl_noop", spl_noop::id(), processor!(spl_noop::noop));

        program_test.add_program(
            "hyperlane_sealevel_test_ism",
            hyperlane_sealevel_test_ism::id(),
            processor!(hyperlane_sealevel_test_ism::program::process_instruction),
        );

        let (banks_client, payer, _) = program_test.start().await;

        (Arc::new(Mutex::new(banks_client)), payer)
    }

    async fn local_validator_client() -> ((RpcClient, RpcSendTransactionConfig), Keypair) {
        let client = RpcClient::new_with_commitment(
            "http://localhost:8899".to_owned(),
            CommitmentConfig::processed(),
        );
        let config = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Processed),
            ..Default::default()
        };
        let payer = Keypair::new();
        client
            .request_airdrop(&payer.pubkey(), 1_000_000_000_000)
            .await
            .unwrap();

        ((client, config), payer)
    }
}
