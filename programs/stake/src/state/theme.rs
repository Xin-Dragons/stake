use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Default)]
pub enum FontFamily {
    Roboto,
    OpenSans,
    Montserrat,
    Lato,
    Poppins,
    #[default]
    SourceSans3,
    LeagueGothic,
    Raleway,
    NotoSans,
    Inter,
    RobotoSlab,
    Merriweather,
    PlayfairDisplay,
    RobotoMono,
    Quattrocento,
    QuattrocentoSans,
    Kanit,
    Nunito,
    WorkSans,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct FontStyles {
    /// The font family (1)
    pub font_family: FontFamily,
    /// bold or normal (1)
    pub bold: bool,
    /// uppercase or normal (1)
    pub uppercase: bool,
}

impl FontStyles {
    pub fn default_header() -> Self {
        Self {
            font_family: FontFamily::default(),
            bold: true,
            uppercase: true,
        }
    }

    pub fn default_body() -> Self {
        Self {
            font_family: FontFamily::default(),
            bold: false,
            uppercase: false,
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Theme {
    /// Link to offchain logo (1 + 1)
    pub logo: Option<u8>,
    /// Link to offchain bg (1 + 1)
    pub background: u8,
    /// All uploaded logos (4)
    pub logos: Vec<String>,
    /// All uploaded bgs (4)
    pub backgrounds: Vec<String>,
    /// Body font styles (3)
    pub body_font: FontStyles,
    /// Header font styles (3)
    pub header_font: FontStyles,
    /// Hexadecimal (string) color (4 + 6)
    pub primary_color: String,
    /// Hexadecimal (string) color (4 + 6)
    pub secondary_color: String,
    /// Whether dark mode is enabled (1)
    pub dark_mode: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            logo: None,
            background: 0,
            logos: vec![],
            backgrounds: vec![
                String::from("/bg.png"),
                String::from("/bg2.png"),
                String::from("/bg3.png"),
                String::from("/bg4.png"),
            ],
            body_font: FontStyles::default_body(),
            header_font: FontStyles::default_header(),
            primary_color: String::from("0BFFD0"),
            secondary_color: String::from("0BFFD0"),
            dark_mode: true,
        }
    }
}

impl Theme {
    pub const LEN: usize = 8 + (1 + 1) + (1 + 1) + 4 + 4 + 3 + 3 + (4 + 6) + (4 + 6) + 1;

    pub const DEFAULT_COLOR: &str = "0BFFD0";

    pub fn init(
        logo: Option<u8>,
        background: u8,
        body_font: FontStyles,
        header_font: FontStyles,
        primary_color: String,
        secondary_color: String,
        dark_mode: bool,
    ) -> Self {
        Self {
            logo,
            background,
            logos: vec![],
            backgrounds: vec![
                String::from("/bg.png"),
                String::from("/bg2.png"),
                String::from("/bg3.png"),
                String::from("/bg4.png"),
            ],
            body_font,
            header_font,
            primary_color,
            secondary_color,
            dark_mode,
        }
    }

    pub fn current_len(&self) -> usize {
        Theme::LEN + self.backgrounds.len() * (4 + 63) + self.logos.len() * (4 + 63)
    }
}
