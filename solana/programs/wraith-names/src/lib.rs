use anchor_lang::prelude::*;

declare_id!("4JrrQh5aK7iLvx6MgtEQk7K7X3SsWfTLxVJu1jXEwNjD");

const MAX_NAME_LEN: usize = 32;

/// PDA-based name registry mapping human-readable names to stealth
/// meta-addresses. Names are validated as 3-32 chars, lowercase alphanumeric
/// or hyphens. Ownership is tied to the registrant's Solana wallet.
///
/// # PDA Authority Model
///
/// Every `NameRecord` lives at a canonical PDA:
///   seeds = [b"name", name.as_bytes()]
///
/// The `owner` field (a Pubkey stored inside the account) controls who may
/// call `update` and `release`. Anchor's `has_one = owner` constraint on
/// those instructions enforces this at the account-validation layer — before
/// any instruction logic runs — so a substituted signer is rejected even if
/// the instruction handler's own check is bypassed.
///
/// # Rent Recovery
///
/// Only the registered owner may close a `NameRecord` via `release`. The
/// `close = owner` attribute transfers the full rent-exempt lamport balance to
/// the `owner` account. The `owner` account must be a signer to prevent an
/// attacker from redirecting lamports to an arbitrary destination: Anchor's
/// account validation ensures the signer's key matches `name_record.owner`
/// before the close happens.
///
/// | Account      | Writable | Signer | Closed by |
/// |--------------|----------|--------|-----------|
/// | name_record  | yes      | no     | release   |
/// | owner        | yes      | yes    | —         |
#[program]
pub mod wraith_names {
    use super::*;

    /// Register a new `.wraith` name with a 64-byte stealth meta-address.
    ///
    /// The PDA is derived as `[b"name", name.as_bytes()]`. Anchor's `init`
    /// constraint guarantees the account does not already exist, so duplicate
    /// registration is impossible at the runtime level.
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
    ///
    /// The `has_one = owner` constraint verifies that `ctx.accounts.owner`
    /// matches `name_record.owner` before entering this handler.
    pub fn update(ctx: Context<Update>, new_meta_address: [u8; 64]) -> Result<()> {
        // has_one = owner already enforced by Anchor at the account-validation
        // layer; this defensive check provides a second barrier.
        require!(
            ctx.accounts.name_record.owner == ctx.accounts.owner.key(),
            WraithError::NotOwner
        );
        ctx.accounts.name_record.meta_address = new_meta_address;
        Ok(())
    }

    /// Release a name, closing the PDA account and returning rent to the owner.
    ///
    /// The `has_one = owner` constraint verifies ownership before the close.
    /// The `close = owner` attribute transfers the full rent balance to the
    /// signer, so the destination cannot be redirected by a third party.
    pub fn release(ctx: Context<Release>) -> Result<()> {
        // has_one = owner already enforced by Anchor; kept for defense-in-depth.
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
    /// This field is enforced by `has_one = owner` on Update and Release.
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
    /// The name record to update. The `has_one` constraint enforces that
    /// `name_record.owner == owner.key()` before the instruction handler runs,
    /// preventing owner-substitution attacks at the account-validation layer.
    #[account(mut, has_one = owner @ WraithError::NotOwner)]
    pub name_record: Account<'info, NameRecord>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct Release<'info> {
    /// The name record to close. `has_one` enforces the owner relationship;
    /// `close = owner` transfers the rent-exempt lamports to the owner.
    /// Because `owner` must be a signer, the rent destination cannot be
    /// redirected to an arbitrary account.
    #[account(mut, has_one = owner @ WraithError::NotOwner, close = owner)]
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
