use anchor_lang::prelude::*;

use crate::{
    state::{FontStyles, Staker},
    StakeError,
};

#[derive(Accounts)]
#[instruction(logo: Option<String>, background: Option<String>)]
pub struct UpdateTheme<'info> {
    #[account(
        mut,
        realloc = staker.current_len() + staker.theme.current_len() + if Option::is_some(&logo) {
            // item already exists, no realloc needed
            if staker.theme.logos.contains(&logo.as_ref().unwrap()) {
                0
            } else {
                4 + 63
            }
        } else {
            0
        } + if Option::is_some(&background) {
            // item already exists, no realloc needed
            if staker.theme.backgrounds.contains(&background.as_ref().unwrap()) {
                0
            } else {
                4 + 63
            }
        } else {
            0
        },
        realloc::payer = authority,
        realloc::zero = false,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn update_theme_handler(
    ctx: Context<UpdateTheme>,
    logo: Option<String>,
    background: Option<String>,
    body_font: Option<FontStyles>,
    header_font: Option<FontStyles>,
    primary_color: Option<String>,
    secondary_color: Option<String>,
    dark_mode: Option<bool>,
) -> Result<()> {
    let staker = &mut ctx.accounts.staker;

    let theme = &mut staker.theme;

    if Option::is_some(&logo) {
        let logo = logo.unwrap();
        if theme.logos.contains(&logo) {
            let index = theme.logos.iter().position(|item| *item == logo).unwrap();
            theme.logo = Some(index as u8);
        } else {
            require!(
                logo.contains("https://arweave.net/"),
                StakeError::InvalidImage
            );
            require_gte!(63, logo.len(), StakeError::ImageTooLong);
            theme.logos.push(logo);
            theme.logo = Some((theme.logos.len() - 1) as u8);
        }
    }

    if Option::is_some(&background) {
        let background = background.unwrap();
        if theme.backgrounds.contains(&background) {
            let index = theme
                .backgrounds
                .iter()
                .position(|item| *item == background)
                .unwrap();
            theme.background = index as u8;
        } else {
            require!(
                background.contains("https://arweave.net/"),
                StakeError::InvalidImage
            );
            require_gte!(63, background.len(), StakeError::ImageTooLong);
            theme.backgrounds.push(background);
            theme.background = (theme.backgrounds.len() - 1) as u8;
        }
    }

    if Option::is_some(&body_font) {
        theme.body_font = body_font.unwrap();
    }

    if Option::is_some(&header_font) {
        theme.header_font = header_font.unwrap();
    }

    if Option::is_some(&primary_color) {
        let primary_color = primary_color.unwrap();
        require_eq!(primary_color.len(), 6, StakeError::InvalidColor);
        require!(
            Option::is_none(&hex::decode(&primary_color).err()),
            StakeError::InvalidColor
        );
        theme.primary_color = primary_color;
    }

    if Option::is_some(&secondary_color) {
        let secondary_color = secondary_color.unwrap();
        require_eq!(secondary_color.len(), 6, StakeError::InvalidColor);
        require!(
            Option::is_none(&hex::decode(&secondary_color).err()),
            StakeError::InvalidColor
        );
        theme.secondary_color = secondary_color;
    }

    if Option::is_some(&dark_mode) {
        theme.dark_mode = dark_mode.unwrap();
    }

    // staker.theme = theme;
    Ok(())
}
