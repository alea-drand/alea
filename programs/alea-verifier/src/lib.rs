#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("FHhKqaQ6k993teCkDrHcXjbpkA4efa7zizuoJcacndNT");

pub mod crypto;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

#[program]
pub mod alea_verifier {}
