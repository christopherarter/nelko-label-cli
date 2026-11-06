use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use nusb::{
    DeviceInfo, MaybeFuture,
    descriptors::TransferType,
    transfer::{Bulk, Direction, Out},
};

fn candidates(selector: Option<&str>) -> Result<Vec<DeviceInfo>> {
    let printers = nusb::list_devices()
        .wait()
        .context("listing USB devices")?
        .filter(|d| d.interfaces().any(|i| i.class() == 7))
        .filter(|d| {
            selector.is_none_or(|query| {
                d.product_string()
                    .is_some_and(|name| normalize(name) == normalize(query))
                    || d.serial_number() == Some(query)
                    || format!("{:04x}:{:04x}", d.vendor_id(), d.product_id())
                        .eq_ignore_ascii_case(query)
            })
        })
        .collect();
    Ok(printers)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn list() -> Result<()> {
    let printers = candidates(None)?;
    if printers.is_empty() {
        println!("No USB printers found. Connect the printer by USB and turn it on.");
    }
    for printer in printers {
        println!(
            "{:04x}:{:04x}\t{}\tserial={}",
            printer.vendor_id(),
            printer.product_id(),
            printer.product_string().unwrap_or("unnamed printer"),
            printer.serial_number().unwrap_or("unavailable")
        );
    }
    Ok(())
}

pub(crate) fn send(selector: &str, job: &[u8]) -> Result<()> {
    let mut printers = candidates(Some(selector))?;
    ensure!(
        printers.len() <= 1,
        "multiple USB printers match {selector}; select a serial number"
    );
    let info = printers
        .pop()
        .with_context(|| format!("USB printer {selector} not found; run `nelko devices`"))?;
    let number = info
        .interfaces()
        .find(|i| i.class() == 7)
        .context("no printer interface")?
        .interface_number();
    let device = info.open().wait().context("opening USB printer")?;
    let interface = device.detach_and_claim_interface(number).wait()
        .context("claiming USB printer interface; close any other printing app and wait for pending jobs to finish")?;
    let address = interface
        .descriptor()
        .context("reading USB interface descriptor")?
        .endpoints()
        .find(|e| e.direction() == Direction::Out && e.transfer_type() == TransferType::Bulk)
        .context("USB printer has no bulk OUT endpoint")?
        .address();
    let mut endpoint = interface
        .endpoint::<Bulk, Out>(address)
        .context("opening USB bulk endpoint")?;
    let mut sent = 0;
    for chunk in job.chunks(16 * 1024) {
        let completion = endpoint.transfer_blocking(chunk.to_vec().into(), Duration::from_secs(10));
        if let Err(error) = completion.status {
            bail!(
                "USB write failed after at least {sent} bytes ({error}); job may be partially received; no automatic retry"
            );
        }
        sent += completion.actual_len;
        ensure!(
            completion.actual_len == chunk.len(),
            "USB short write after {sent} bytes; job may be partially received; no automatic retry"
        );
    }
    Ok(())
}
