use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Token, TokenAccount};

declare_id!("E6J7GBSTjKbYANWjfTo5HfnXZ4Tg3LAasN7NrvCwn5Dq");

/// Atomic send-and-announce program. Transfers SOL or SPL tokens to a stealth
/// address and emits an announcement event in a single transaction.
#[program]
pub mod wraith_sender {
    use super::*;

    /// Transfer SOL to a stealth address and emit an announcement.
    pub fn send_sol(
        ctx: Context<SendSol>,
        amount: u64,
        scheme_id: u32,
        stealth_address: Pubkey,
        ephemeral_pub_key: [u8; 32],
        metadata: Vec<u8>,
    ) -> Result<()> {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.sender.to_account_info(),
                    to: ctx.accounts.stealth_account.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(AnnouncementEvent {
            scheme_id,
            stealth_address,
            caller: ctx.accounts.sender.key(),
            ephemeral_pub_key,
            metadata,
        });

        Ok(())
    }

    /// Transfer SPL tokens to a stealth address's token account and emit an announcement.
    pub fn send_spl(
        ctx: Context<SendSpl>,
        amount: u64,
        scheme_id: u32,
        stealth_address: Pubkey,
        ephemeral_pub_key: [u8; 32],
        metadata: Vec<u8>,
    ) -> Result<()> {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.sender_token_account.to_account_info(),
                    to: ctx.accounts.stealth_token_account.to_account_info(),
                    authority: ctx.accounts.sender.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(AnnouncementEvent {
            scheme_id,
            stealth_address,
            caller: ctx.accounts.sender.key(),
            ephemeral_pub_key,
            metadata,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct SendSol<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    /// CHECK: stealth address, receives SOL
    #[account(mut)]
    pub stealth_account: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SendSpl<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    #[account(mut)]
    pub sender_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub stealth_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[event]
pub struct AnnouncementEvent {
    pub scheme_id: u32,
    pub stealth_address: Pubkey,
    pub caller: Pubkey,
    pub ephemeral_pub_key: [u8; 32],
    pub metadata: Vec<u8>,
}
