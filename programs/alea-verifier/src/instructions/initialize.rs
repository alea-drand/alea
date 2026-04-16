use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_handler(
    _ctx: Context<Initialize>,
    _pubkey_g2: [u8; 128],
    _genesis_time: u64,
    _period: u64,
    _chain_hash: [u8; 32],
) -> Result<()> {
    Ok(())
}
