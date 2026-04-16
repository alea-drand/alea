// TEMPORARY — Phase 1.1.B probe. Remove in Phase 2.
use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_compute_units;
use ark_bn254::G2Affine;
use ark_ec::AffineRepr;
use ark_serialize::CanonicalDeserialize;

#[derive(Accounts)]
pub struct ProbeG2<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
}

pub fn handler(_ctx: Context<ProbeG2>, pubkey_bytes: Vec<u8>) -> Result<()> {
    require!(pubkey_bytes.len() == 128, ErrorCode::ConstraintRaw);

    sol_log_compute_units();

    let point = deserialize_g2_eip197(&pubkey_bytes)
        .map_err(|_| error!(ErrorCode::ConstraintRaw))?;

    let on_curve = point.is_on_curve();
    let in_subgroup = point.is_in_correct_subgroup_assuming_on_curve();

    sol_log_compute_units();

    msg!("on_curve={}, in_subgroup={}", on_curve, in_subgroup);
    Ok(())
}

fn deserialize_g2_eip197(bytes: &[u8]) -> std::result::Result<G2Affine, ()> {
    // EIP-197: x_c1(32) || x_c0(32) || y_c1(32) || y_c0(32), all big-endian
    // ark-bn254 uses CanonicalDeserialize which expects little-endian.
    // Try the canonical format first; if that doesn't match, we'll convert.
    let mut le_bytes = Vec::with_capacity(128);
    // Convert each 32-byte chunk from BE to LE
    for chunk in bytes.chunks(32) {
        let mut reversed = chunk.to_vec();
        reversed.reverse();
        le_bytes.extend_from_slice(&reversed);
    }
    // ark expects: x_c0(32LE) || x_c1(32LE) || y_c0(32LE) || y_c1(32LE)
    // EIP-197 is: x_c1(32BE) || x_c0(32BE) || y_c1(32BE) || y_c0(32BE)
    // So after BE→LE conversion: x_c1(32LE) || x_c0(32LE) || y_c1(32LE) || y_c0(32LE)
    // Need to swap c0/c1 pairs: [c1,c0] → [c0,c1]
    let mut ark_bytes = vec![0u8; 128];
    ark_bytes[0..32].copy_from_slice(&le_bytes[32..64]);   // x_c0
    ark_bytes[32..64].copy_from_slice(&le_bytes[0..32]);   // x_c1
    ark_bytes[64..96].copy_from_slice(&le_bytes[96..128]);  // y_c0
    ark_bytes[96..128].copy_from_slice(&le_bytes[64..96]);  // y_c1

    G2Affine::deserialize_uncompressed(&ark_bytes[..]).map_err(|_| ())
}
