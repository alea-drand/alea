#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U");

pub mod crypto;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

#[program]
pub mod alea_verifier {}
