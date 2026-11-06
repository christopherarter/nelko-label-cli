//! Print PDF shipping labels through direct USB or Bluetooth connections.

mod bluetooth;
mod pdf;
mod profile;
mod protocol;
mod raster;
mod usb;

use std::{path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use raster::Label;

#[derive(Debug, Parser)]
#[command(
    name = "open-nelko",
    version,
    about = "Print PDF labels to a Nelko PL70e-BT over USB or Bluetooth",
    after_help = "Independent hobby project; unaffiliated with Nelko.\nMIT licensed. Provided AS IS, without warranty or guaranteed support."
)]
struct Cli {
    /// Printer model profile; use `profiles` to list supported models.
    #[arg(long, global = true, default_value = "nelko-pl70e-bt")]
    profile: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List built-in printer profiles and their media settings.
    Profiles,
    /// List USB printers or scan for Bluetooth printers. Never sends print data.
    Devices {
        #[arg(long, value_enum, default_value_t = Transport::Usb)]
        transport: Transport,
        #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u64).range(1..=60))]
        seconds: u64,
        /// Include Bluetooth devices whose names do not look like printers.
        #[arg(long)]
        all: bool,
    },
    /// Show a Bluetooth printer's services and characteristics. Does not print.
    Inspect {
        #[arg(long)]
        device: Option<String>,
        #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u64).range(1..=60))]
        seconds: u64,
    },
    /// Query TSPL printer status over Bluetooth without printing a label.
    Status {
        #[arg(long)]
        device: Option<String>,
        #[arg(long, value_parser = uuid::Uuid::parse_str)]
        characteristic: Option<uuid::Uuid>,
        #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u64).range(1..=60))]
        seconds: u64,
    },
    /// Render exactly the monochrome image that will be sent to the printer.
    Preview {
        #[command(flatten)]
        input: Input,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Save a TSPL print job for inspection without contacting hardware.
    Export {
        #[command(flatten)]
        input: Input,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Print the selected PDF page using an explicitly selected connection.
    Print {
        #[command(flatten)]
        input: Input,
        #[arg(long, value_enum)]
        transport: Transport,
        /// Exact advertised name, peripheral ID, or USB serial from `devices`.
        #[arg(long)]
        device: Option<String>,
        /// Override automatic Bluetooth characteristic selection with a UUID.
        #[arg(long, value_parser = uuid::Uuid::parse_str)]
        characteristic: Option<uuid::Uuid>,
        #[arg(long, default_value_t = 180, value_parser = clap::value_parser!(u16).range(1..=512))]
        chunk_size: u16,
        #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u64).range(1..=1000))]
        delay_ms: u64,
        #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u64).range(1..=60))]
        seconds: u64,
        /// Validate and describe the job without discovering or writing to devices.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Transport {
    Usb,
    Bluetooth,
}

#[derive(Debug, Args)]
struct Input {
    /// A PDF file. One page is printed per job.
    file: PathBuf,
    /// One-based page number. Required for PDFs with more than one page.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    page: Option<u32>,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u16).range(1..=100))]
    copies: u16,
    /// Pixel threshold: values below it become black.
    #[arg(long, default_value_t = 160, value_parser = clap::value_parser!(u8).range(1..=255))]
    threshold: u8,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn render(input: &Input, profile: &profile::PrinterProfile) -> Result<Label> {
    let gray = pdf::render(&input.file, input.page, profile.media)?;
    Label::from_gray(gray, profile.media, input.threshold, input.copies)
}

async fn run(cli: Cli) -> Result<()> {
    let profile = profile::named(&cli.profile)?;
    match cli.command {
        Command::Profiles => {
            for profile in profile::ALL {
                println!(
                    "{}\t{}\t{}×{} inches\t{} dpi",
                    profile.id,
                    profile.name,
                    profile.media.width_inches,
                    profile.media.height_inches,
                    profile.media.dpi
                );
            }
            Ok(())
        }
        Command::Devices {
            transport,
            seconds,
            all,
        } => match transport {
            Transport::Usb => usb::list(),
            Transport::Bluetooth => bluetooth::list(seconds, all, profile.device_name).await,
        },
        Command::Inspect { device, seconds } => {
            bluetooth::inspect(device.as_deref().unwrap_or(profile.device_name), seconds).await
        }
        Command::Status {
            device,
            characteristic,
            seconds,
        } => {
            bluetooth::status(
                device.as_deref().unwrap_or(profile.device_name),
                seconds,
                characteristic,
                profile
                    .bluetooth
                    .context("selected profile does not support Bluetooth")?,
            )
            .await
        }
        Command::Preview { input, output } => {
            render(&input, profile)?.save_png(&output)?;
            let (width, height) = profile.media.dimensions();
            println!(
                "Saved {width}×{height} monochrome preview to {}",
                output.display()
            );
            Ok(())
        }
        Command::Export { input, output } => {
            std::fs::write(
                &output,
                protocol::encode(profile.language, &render(&input, profile)?),
            )?;
            println!("Saved TSPL job to {}", output.display());
            Ok(())
        }
        Command::Print {
            input,
            transport,
            device,
            characteristic,
            chunk_size,
            delay_ms,
            seconds,
            dry_run,
        } => {
            let device = device.as_deref().unwrap_or(profile.device_name);
            if matches!(transport, Transport::Usb) && characteristic.is_some() {
                anyhow::bail!("--characteristic is only valid with --transport bluetooth");
            }
            let label = render(&input, profile)?;
            let job = protocol::encode(profile.language, &label);
            let (width, height) = profile.media.dimensions();
            if dry_run {
                println!(
                    "Dry run: {} copy/copies, {}×{} inches, {} dpi, {width}×{height} dots, {} bytes over {transport:?} to {device}. No device was contacted.",
                    input.copies,
                    profile.media.width_inches,
                    profile.media.height_inches,
                    profile.media.dpi,
                    job.len()
                );
                return Ok(());
            }
            match transport {
                Transport::Usb => usb::send(device, &job)?,
                Transport::Bluetooth => {
                    bluetooth::send(
                        device,
                        seconds,
                        characteristic,
                        &job,
                        chunk_size,
                        delay_ms,
                        profile
                            .bluetooth
                            .context("selected profile does not support Bluetooth")?,
                    )
                    .await?
                }
            }
            println!(
                "Sent {} bytes ({} copy/copies) to {device} over {transport:?}. Check the printed label; the transport does not confirm physical printing.",
                job.len(),
                input.copies
            );
            Ok(())
        }
    }
}
