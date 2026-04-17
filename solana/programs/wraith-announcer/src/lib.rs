use anchor_lang::prelude::*;

declare_id!("9Ko7TuXHpLUH1ZsZWQEpeA9Tv7hX325ooWk5SD7Y9nuq");

/// Stateless stealth address announcer. Emits events that indexers watch to
/// let recipients detect incoming payments. No storage, no access control.
#[program]
pub mod wraith_announcer {
    use super::*;

    /// Publish a stealth address announcement event.
    ///
    /// # Arguments
    /// * `scheme_id` – Stealth address scheme (1 = default DKSAP).
    /// * `stealth_address` – One-time address that received funds.
    /// * `ephemeral_pub_key` – Ephemeral key used to derive the stealth address.
    /// * `metadata` – Arbitrary bytes (first byte is the view tag).
    pub fn announce(
        ctx: Context<Announce>,
        scheme_id: u32,
        stealth_address: Pubkey,
        ephemeral_pub_key: [u8; 32],
        metadata: Vec<u8>,
    ) -> Result<()> {
        emit!(AnnouncementEvent {
            scheme_id,
            stealth_address,
            caller: ctx.accounts.caller.key(),
            ephemeral_pub_key,
            metadata,
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Announce<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,
}

#[event]
pub struct AnnouncementEvent {
    pub scheme_id: u32,
    pub stealth_address: Pubkey,
    pub caller: Pubkey,
    pub ephemeral_pub_key: [u8; 32],
    pub metadata: Vec<u8>,
}
