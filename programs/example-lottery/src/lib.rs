// Placeholder — pilot step 5: verify BPF compilation with alea-sdk path dep.
// Full implementation lands in later commit.

use anchor_lang::prelude::*;

declare_id!("ExLotTerY1111111111111111111111111111111111");

#[program]
pub mod example_lottery {
    use super::*;

    pub fn ping(_ctx: Context<Ping>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Ping {}
