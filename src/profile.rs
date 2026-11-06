use anyhow::{Result, bail};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Media {
    pub width_inches: f64,
    pub height_inches: f64,
    pub dpi: u16,
}

impl Media {
    pub(crate) fn dimensions(self) -> (usize, usize) {
        (
            (self.width_inches * f64::from(self.dpi)).round() as usize,
            (self.height_inches * f64::from(self.dpi)).round() as usize,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Language {
    Tspl,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BluetoothChannel {
    pub service: Uuid,
    pub characteristic: Uuid,
}

#[derive(Debug)]
pub(crate) struct PrinterProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub device_name: &'static str,
    pub media: Media,
    pub language: Language,
    pub bluetooth: Option<BluetoothChannel>,
}

pub(crate) const NELKO: PrinterProfile = PrinterProfile {
    id: "nelko-pl70e-bt",
    name: "Nelko PL70e-BT",
    device_name: "PL70e-BT",
    media: Media {
        width_inches: 4.0,
        height_inches: 6.0,
        dpi: 203,
    },
    language: Language::Tspl,
    bluetooth: Some(BluetoothChannel {
        service: Uuid::from_u128(0x0000fff0_0000_1000_8000_00805f9b34fb),
        characteristic: Uuid::from_u128(0x0000fff2_0000_1000_8000_00805f9b34fb),
    }),
};

pub(crate) const ALL: &[PrinterProfile] = &[NELKO];

pub(crate) fn named(id: &str) -> Result<&'static PrinterProfile> {
    if let Some(profile) = ALL.iter().find(|profile| profile.id == id) {
        return Ok(profile);
    }
    bail!("unknown printer profile {id}; run `open-nelko profiles`")
}
