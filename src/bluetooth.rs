use crate::profile::BluetoothChannel;
use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use btleplug::{
    api::{
        Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
        WriteType,
    },
    platform::{Adapter, Manager, Peripheral},
};
use futures_util::StreamExt;
use tokio::time;
use uuid::Uuid;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);

async fn scan(seconds: u64) -> Result<(Adapter, Vec<Peripheral>)> {
    let manager = time::timeout(OPERATION_TIMEOUT, Manager::new()).await
        .context("Bluetooth initialization timed out; allow Bluetooth access in System Settings → Privacy & Security → Bluetooth")??;
    let adapter = manager
        .adapters()
        .await?
        .into_iter()
        .next()
        .context("no Bluetooth adapter found")?;
    time::timeout(OPERATION_TIMEOUT, adapter.start_scan(ScanFilter::default()))
        .await
        .context("Bluetooth scan timed out; check Bluetooth permissions")??;
    time::sleep(Duration::from_secs(seconds)).await;
    let peripherals = adapter.peripherals().await;
    adapter
        .stop_scan()
        .await
        .context("stopping Bluetooth scan")?;
    Ok((adapter, peripherals?))
}

fn normalized(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) async fn list(seconds: u64, all: bool, name_prefix: &str) -> Result<()> {
    eprintln!("Scanning Bluetooth for {seconds}s; macOS may ask for Bluetooth access...");
    let (_adapter, peripherals) = scan(seconds).await?;
    let mut rows = Vec::new();
    for peripheral in peripherals {
        let properties = peripheral.properties().await?;
        let name = properties
            .as_ref()
            .and_then(|p| p.local_name.as_deref())
            .unwrap_or("unnamed");
        let lower = name.to_ascii_lowercase();
        if all
            || normalized(name).starts_with(&normalized(name_prefix))
            || lower.contains("printer")
        {
            rows.push(format!(
                "{}\t{name}\tRSSI={}",
                peripheral.id(),
                properties
                    .as_ref()
                    .and_then(|p| p.rssi)
                    .map_or("?".to_owned(), |r| r.to_string())
            ));
        }
    }
    rows.sort();
    if rows.is_empty() {
        println!(
            "No matching Bluetooth printers found. Turn it on, disconnect other apps, or retry with --all."
        );
    }
    for row in rows {
        println!("{row}");
    }
    Ok(())
}

async fn connect(selector: &str, seconds: u64) -> Result<(Adapter, Peripheral)> {
    let (adapter, peripherals) = scan(seconds).await?;
    let mut matches = Vec::new();
    for peripheral in peripherals {
        let props = peripheral.properties().await?;
        let name_matches = props.and_then(|p| p.local_name).is_some_and(|name| {
            let expected = normalized(selector);
            let actual = normalized(&name);
            !expected.is_empty() && actual.starts_with(&expected)
        });
        if name_matches || peripheral.id().to_string().eq_ignore_ascii_case(selector) {
            matches.push(peripheral);
        }
    }
    ensure!(
        matches.len() <= 1,
        "multiple Bluetooth printers match {selector}; use a peripheral ID from `open-nelko devices --transport bluetooth`"
    );
    let peripheral = matches.pop().with_context(|| format!("Bluetooth printer {selector} not found; turn it on and run `open-nelko devices --transport bluetooth`"))?;
    time::timeout(OPERATION_TIMEOUT, peripheral.connect())
        .await
        .context("Bluetooth connection timed out")??;
    if let Err(error) = time::timeout(OPERATION_TIMEOUT, peripheral.discover_services())
        .await
        .context("Bluetooth discovery timed out")
        .and_then(|r| r.map_err(Into::into))
    {
        let _ = time::timeout(Duration::from_secs(3), peripheral.disconnect()).await;
        return Err(error);
    }
    Ok((adapter, peripheral))
}

pub(crate) async fn inspect(selector: &str, seconds: u64) -> Result<()> {
    let (_adapter, peripheral) = connect(selector, seconds).await?;
    println!("Bluetooth device: {}", peripheral.id());
    for service in peripheral.services() {
        println!("service {}", service.uuid);
        for characteristic in service.characteristics {
            println!("  {} {:?}", characteristic.uuid, characteristic.properties);
        }
    }
    time::timeout(Duration::from_secs(3), peripheral.disconnect())
        .await
        .context("Bluetooth disconnect timed out")??;
    Ok(())
}

fn choose_characteristic(
    chars: impl IntoIterator<Item = Characteristic>,
    requested: Option<Uuid>,
    preferred: BluetoothChannel,
) -> Result<Characteristic> {
    // Common transparent serial channels on TSPL BLE printer modules. Never guess
    // among arbitrary writable characteristics (some are firmware control points).
    const SERIAL_CHARACTERISTICS: [&str; 3] = [
        "0000ff02-0000-1000-8000-00805f9b34fb",
        "0000fff2-0000-1000-8000-00805f9b34fb",
        "49535343-8841-43f4-a8d4-ecbe34729bb3",
    ];
    let mut matches: Vec<_> = chars
        .into_iter()
        .filter(|c| {
            c.properties
                .intersects(CharPropFlags::WRITE | CharPropFlags::WRITE_WITHOUT_RESPONSE)
                && requested.map_or_else(
                    || {
                        SERIAL_CHARACTERISTICS
                            .iter()
                            .any(|id| c.uuid.to_string() == *id)
                    },
                    |id| c.uuid == id,
                )
        })
        .collect();
    if requested.is_none() {
        // PL70e-BT revisions can expose both FFF0 and an ISSC UART service.
        // FFF0/FFF2 has been verified with this printer's TSPL status reply.
        let profile = matches.iter().position(|c| {
            c.uuid == preferred.characteristic && c.service_uuid == preferred.service
        });
        if let Some(index) = profile {
            let same_channel = matches
                .iter()
                .filter(|c| {
                    c.uuid == matches[index].uuid && c.service_uuid == matches[index].service_uuid
                })
                .count();
            ensure!(
                same_channel == 1,
                "duplicate Bluetooth print channels found"
            );
            return Ok(matches.swap_remove(index));
        }
    }
    ensure!(
        matches.len() <= 1,
        "multiple Bluetooth serial channels found; use `inspect` and select --characteristic UUID"
    );
    matches.pop().context("no recognized Bluetooth print characteristic; use `open-nelko inspect` and specify --characteristic UUID after identifying the TSPL data channel")
}

pub(crate) async fn status(
    selector: &str,
    seconds: u64,
    requested: Option<Uuid>,
    preferred: BluetoothChannel,
) -> Result<()> {
    let (_adapter, peripheral) = connect(selector, seconds).await?;
    let result = async {
        let chars = peripheral.characteristics();
        let tx = choose_characteristic(chars.iter().cloned(), requested, preferred)?;
        let rx = chars
            .iter()
            .find(|c| {
                c.service_uuid == tx.service_uuid
                    && c.properties
                        .intersects(CharPropFlags::NOTIFY | CharPropFlags::INDICATE)
            })
            .context("no status notification characteristic on selected service")?;
        let mut notifications = peripheral.notifications().await?;
        peripheral.subscribe(rx).await?;
        // TSPL's ESC ! ? is a status query, not a print or configuration command.
        let write_type = if tx.properties.contains(CharPropFlags::WRITE) {
            WriteType::WithResponse
        } else {
            WriteType::WithoutResponse
        };
        time::timeout(
            Duration::from_secs(5),
            peripheral.write(&tx, &[0x1b, 0x21, 0x3f], write_type),
        )
        .await
        .context("status query write timed out")??;
        let notification = time::timeout(Duration::from_secs(5), async {
            while let Some(n) = notifications.next().await {
                if n.uuid == rx.uuid {
                    return Some(n);
                }
            }
            None
        })
        .await
        .context("printer did not reply to the TSPL status query")?
        .context("Bluetooth notification stream ended")?;
        println!(
            "TSPL status reply from {}: {:02x?}",
            notification.uuid, notification.value
        );
        if notification.value == [0] {
            println!("Printer reports ready.");
        }
        peripheral.unsubscribe(rx).await?;
        Ok(())
    }
    .await;
    let _ = time::timeout(Duration::from_secs(3), peripheral.disconnect()).await;
    result
}

pub(crate) async fn send(
    selector: &str,
    seconds: u64,
    requested: Option<Uuid>,
    job: &[u8],
    chunk_size: u16,
    delay_ms: u64,
    preferred: BluetoothChannel,
) -> Result<()> {
    let (_adapter, peripheral) = connect(selector, seconds).await?;
    let result = async {
        let characteristic = choose_characteristic(peripheral.characteristics(), requested, preferred)?;
        let write_type = if characteristic.properties.contains(CharPropFlags::WRITE) { WriteType::WithResponse } else { WriteType::WithoutResponse };
        let chunk_size = usize::from(chunk_size.min(peripheral.mtu().saturating_sub(3).max(1)));
        eprintln!("Sending {} bytes via {} in {chunk_size}-byte chunks ({write_type:?})...", job.len(), characteristic.uuid);
        let mut sent = 0;
        for chunk in job.chunks(chunk_size) {
            let outcome = time::timeout(Duration::from_secs(10), peripheral.write(&characteristic, chunk, write_type)).await;
            match outcome {
                Ok(Ok(())) => sent += chunk.len(),
                Ok(Err(error)) => bail!("Bluetooth write failed after at least {sent} bytes ({error}); job may be partially received; no automatic retry"),
                Err(_) => bail!("Bluetooth write timed out after at least {sent} bytes; job may be partially received; no automatic retry"),
            }
            time::sleep(Duration::from_millis(delay_ms)).await;
        }
        // Especially for writes without response, let the controller drain before disconnecting.
        time::sleep(Duration::from_secs(2)).await;
        Ok(())
    }.await;
    let disconnect = time::timeout(Duration::from_secs(3), peripheral.disconnect()).await;
    result?;
    disconnect
        .context("job sent, but Bluetooth disconnect timed out; do not automatically reprint")?
        .context("job sent, but Bluetooth disconnect failed; do not automatically reprint")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn characteristic(id: &str) -> Characteristic {
        Characteristic {
            uuid: Uuid::parse_str(id).expect("test UUID is valid"),
            service_uuid: Uuid::nil(),
            properties: CharPropFlags::WRITE,
            descriptors: Default::default(),
        }
    }

    #[test]
    fn refuses_unknown_or_ambiguous_channels() {
        let a = characteristic("0000ff02-0000-1000-8000-00805f9b34fb");
        let b = characteristic("0000fff2-0000-1000-8000-00805f9b34fb");
        let unknown = characteristic("00001234-0000-1000-8000-00805f9b34fb");
        assert!(
            choose_characteristic(
                [unknown.clone()],
                None,
                crate::profile::NELKO
                    .bluetooth
                    .expect("Nelko Bluetooth profile")
            )
            .is_err()
        );
        assert!(
            choose_characteristic(
                [a.clone(), b],
                None,
                crate::profile::NELKO
                    .bluetooth
                    .expect("Nelko Bluetooth profile")
            )
            .is_err()
        );
        assert_eq!(
            choose_characteristic(
                [a.clone()],
                None,
                crate::profile::NELKO
                    .bluetooth
                    .expect("Nelko Bluetooth profile")
            )
            .expect("one known serial channel")
            .uuid,
            a.uuid
        );
        assert_eq!(
            choose_characteristic(
                [unknown.clone()],
                Some(unknown.uuid),
                crate::profile::NELKO
                    .bluetooth
                    .expect("Nelko Bluetooth profile")
            )
            .expect("explicit channel")
            .uuid,
            unknown.uuid
        );
    }

    #[test]
    fn prefers_verified_nelko_channel_when_both_services_exist() {
        let mut nelko = characteristic("0000fff2-0000-1000-8000-00805f9b34fb");
        nelko.service_uuid = btleplug::api::bleuuid::uuid_from_u16(0xfff0);
        let uart = characteristic("49535343-8841-43f4-a8d4-ecbe34729bb3");
        assert_eq!(
            choose_characteristic(
                [uart, nelko.clone()],
                None,
                crate::profile::NELKO
                    .bluetooth
                    .expect("Nelko Bluetooth profile")
            )
            .expect("Nelko profile")
            .uuid,
            nelko.uuid
        );
    }
}
