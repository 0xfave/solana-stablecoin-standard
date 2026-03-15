use anchor_lang::prelude::*;

#[account]
pub struct ExtraAccountMetaListAccount {
    pub bump: u8,
}

impl ExtraAccountMetaListAccount {
    pub const INIT_SPACE: usize = 51; // ExtraAccountMetaList::size_of(1) = 51
}
