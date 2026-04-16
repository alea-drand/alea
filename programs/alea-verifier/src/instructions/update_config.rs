use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    pub authority: Signer<'info>,
}

pub fn update_config_handler(
    _ctx: Context<UpdateConfig>,
    _pubkey_g2: [u8; 128],
    _genesis_time: u64,
    _period: u64,
    _chain_hash: [u8; 32],
) -> Result<()> {
    Ok(())
}
