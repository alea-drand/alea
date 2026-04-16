use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Verify<'info> {
    pub payer: Signer<'info>,
}

pub fn verify_handler(
    _ctx: Context<Verify>,
    _round: u64,
    _signature: [u8; 64],
) -> Result<[u8; 32]> {
    Ok([0u8; 32])
}
