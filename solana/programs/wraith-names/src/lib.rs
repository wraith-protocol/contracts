use anchor_lang::prelude::*;

declare_id!("4JrrQh5aK7iLvx6MgtEQk7K7X3SsWfTLxVJu1jXEwNjD");

const MAX_NAME_LEN: usize = 32;

/// PDA-based name registry mapping human-readable names to stealth
/// meta-addresses. Names are validated as 3-32 chars, lowercase alphanumeric
/// or hyphens. Ownership is tied to the registrant's Solana wallet.
#[program]
pub mod wraith_names {
    use super::*;

    /// Register a new `.wraith` name with a 64-byte stealth meta-address.
    pub fn register(
        ctx: Context<Register>,
        name: String,
        meta_address: [u8; 64],
    ) -> Result<()> {
        require!(
            name.len() >= 3 && name.len() <= MAX_NAME_LEN,
            WraithError::InvalidNameLength
        );
        require!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            WraithError::InvalidNameCharacter
        );

        let record = &mut ctx.accounts.name_record;
        record.name = name;
        record.meta_address = meta_address;
        record.owner = ctx.accounts.owner.key();
        record.created_at = Clock::get()?.unix_timestamp;

        Ok(())
    }

    /// Update the meta-address for an existing name. Only the owner may call.
    pub fn update(ctx: Context<Update>, new_meta_address: [u8; 64]) -> Result<()> {
        require!(
            ctx.accounts.name_record.owner == ctx.accounts.owner.key(),
            WraithError::NotOwner
        );
        ctx.accounts.name_record.meta_address = new_meta_address;
        Ok(())
    }

    /// Release a name, closing the PDA account and returning rent to the owner.
    pub fn release(ctx: Context<Release>) -> Result<()> {
        require!(
            ctx.accounts.name_record.owner == ctx.accounts.owner.key(),
            WraithError::NotOwner
        );
        Ok(())
    }

    /// Resolve a name to its 64-byte stealth meta-address.
    pub fn resolve(ctx: Context<Resolve>) -> Result<[u8; 64]> {
        Ok(ctx.accounts.name_record.meta_address)
    }
}

#[account]
pub struct NameRecord {
    /// The human-readable name (max 32 bytes).
    pub name: String,
    /// 64-byte stealth meta-address: spending_pub || viewing_pub.
    pub meta_address: [u8; 64],
    /// The wallet that owns this name registration.
    pub owner: Pubkey,
    /// Unix timestamp of registration.
    pub created_at: i64,
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Register<'info> {
    #[account(
        init,
        payer = owner,
        space = 8 + 4 + MAX_NAME_LEN + 64 + 32 + 8,
        seeds = [b"name", name.as_bytes()],
        bump,
    )]
    pub name_record: Account<'info, NameRecord>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub name_record: Account<'info, NameRecord>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct Release<'info> {
    #[account(mut, close = owner)]
    pub name_record: Account<'info, NameRecord>,
    #[account(mut)]
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct Resolve<'info> {
    pub name_record: Account<'info, NameRecord>,
}

#[error_code]
pub enum WraithError {
    #[msg("Name must be 3-32 characters")]
    InvalidNameLength,
    #[msg("Name must be lowercase alphanumeric or hyphens")]
    InvalidNameCharacter,
    #[msg("Only the owner can modify this name")]
    NotOwner,
}
