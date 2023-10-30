use hyperlane_core::{Encode, HyperlaneMessage};
use hyperlane_sealevel_mailbox::{
    instruction::{
        InboxProcess, Init as InitMailbox, Instruction as MailboxInstruction, OutboxDispatch,
    },
    mailbox_dispatched_message_pda_seeds, mailbox_inbox_pda_seeds, mailbox_outbox_pda_seeds,
    mailbox_process_authority_pda_seeds, mailbox_processed_message_pda_seeds, spl_noop,
};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
};

pub struct MailboxAccounts {
    pub program: Pubkey,
    pub inbox: Pubkey,
    pub inbox_bump_seed: u8,
    pub outbox: Pubkey,
    pub outbox_bump_seed: u8,
    pub default_ism: Pubkey,
}

/// Create a mailbox
pub fn initialize_mailbox(
    mailbox_program_id: &Pubkey,
    payer: Pubkey,
    local_domain: u32,
    default_ism: Pubkey,
) -> (Instruction, MailboxAccounts) {
    let (inbox_account, inbox_bump) =
        Pubkey::find_program_address(mailbox_inbox_pda_seeds!(), mailbox_program_id);
    let (outbox_account, outbox_bump) =
        Pubkey::find_program_address(mailbox_outbox_pda_seeds!(), mailbox_program_id);

    let ixn = MailboxInstruction::Init(InitMailbox {
        local_domain,
        default_ism,
    });
    let init_instruction = Instruction {
        program_id: *mailbox_program_id,
        data: ixn.into_instruction_data().unwrap(),
        accounts: vec![
            AccountMeta::new(system_program::id(), false),
            AccountMeta::new(payer, true),
            AccountMeta::new(inbox_account, false),
            AccountMeta::new(outbox_account, false),
        ],
    };

    (
        init_instruction,
        MailboxAccounts {
            program: *mailbox_program_id,
            inbox: inbox_account,
            inbox_bump_seed: inbox_bump,
            outbox: outbox_account,
            outbox_bump_seed: outbox_bump,
            default_ism,
        },
    )
}

/// Add a message to a mailbox's outbox, to be relayed to another mailbox.
/// 
/// Returns
///   - the instruction
///   - a keypair that must sign the transaction
///   - the address to the HyperlaneMessage
pub fn outbox_dispatch(
    mailbox_program_id: &Pubkey,
    outbox: &Pubkey,
    payer: &Pubkey,
    message: OutboxDispatch,
) -> (Instruction, Keypair, Pubkey) {
    let unique_message_account_keypair = Keypair::new();
    let (dispatched_message_account_key, _dispatched_message_bump) = Pubkey::find_program_address(
        mailbox_dispatched_message_pda_seeds!(&unique_message_account_keypair.pubkey()),
        mailbox_program_id,
    );

    (
        Instruction {
            program_id: *mailbox_program_id,
            accounts: vec![
                AccountMeta::new(*outbox, false),
                AccountMeta::new(message.sender, true),
                AccountMeta::new_readonly(system_program::id(), false),
                AccountMeta::new_readonly(spl_noop::id(), false),
                AccountMeta::new(*payer, true),
                AccountMeta::new(unique_message_account_keypair.pubkey(), true),
                AccountMeta::new(dispatched_message_account_key, false),
            ],
            data: MailboxInstruction::OutboxDispatch(message)
                .into_instruction_data()
                .unwrap(),
        },
        unique_message_account_keypair,
        dispatched_message_account_key,
    )
}

/// Relay a message from another mailbox's outbox into the inbox.
pub fn inbox_process(
    mailbox_program_id: &Pubkey,
    inbox: &Pubkey,
    payer: &Pubkey,
    metadata: Vec<u8>,
    message: &HyperlaneMessage,
    get_ism: Instruction,
    ism_verify: Instruction,
    recipient_handle: Instruction,
) -> Instruction {
    // accounts
    let recipient = recipient_handle.program_id;
    let process_authority_key = Pubkey::find_program_address(
        mailbox_process_authority_pda_seeds!(&recipient),
        &mailbox_program_id,
    );
    let processed_message_account_key = Pubkey::find_program_address(
        mailbox_processed_message_pda_seeds!(message.id()),
        &mailbox_program_id,
    );
    let accounts = [
        vec![
            AccountMeta::new_readonly(*payer, true),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new(*inbox, false),
            AccountMeta::new_readonly(process_authority_key.0, false),
            AccountMeta::new(processed_message_account_key.0, false),
        ],
        get_ism.accounts,
        vec![
            AccountMeta::new_readonly(spl_noop::id(), false),
            AccountMeta::new_readonly(ism_verify.program_id, false),
        ],
        ism_verify.accounts,
        vec![AccountMeta::new_readonly(recipient, false)],
        recipient_handle.accounts,
    ];

    // data
    let mut encoded_message = vec![];
    message.write_to(&mut encoded_message).unwrap();
    let ixn = MailboxInstruction::InboxProcess(InboxProcess {
        metadata: metadata.to_vec(),
        message: encoded_message,
    });
    let ixn_data = ixn.into_instruction_data().unwrap();

    Instruction {
        program_id: *mailbox_program_id,
        data: ixn_data,
        accounts: accounts.into_iter().flatten().collect(),
    }
}

/// An instruction with no accounts or data
pub fn empty(program_id: Pubkey) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![],
        data: vec![],
    }
}
