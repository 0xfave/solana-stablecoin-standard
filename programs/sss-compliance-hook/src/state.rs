use anchor_lang::prelude::*;

pub const SEED_EXTRA_ACCOUNT_META_LIST: &[u8] = b"extra-account-metas";

#[account]
#[derive(InitSpace)]
pub struct ExtraAccountMetaListAccount {
    #[max_len(1024)]
    pub data: Vec<u8>,
}
