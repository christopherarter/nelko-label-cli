# Adding printer support

The CLI uses this path:

```text
PDF → monochrome Label → LabelEncoder → command bytes → USB or Bluetooth
              ↑                ↑                         ↑
        media and DPI     printer language          device/channel
                       PrinterProfile
```

The renderer and preview do not contain TSPL commands. Transport modules do not encode page contents. A profile connects a model's default media, resolution, command language and Bluetooth channel.

| Printer difference | Change |
| --- | --- |
| Same TSPL bitmap protocol, another resolution or label size | Add a `PrinterProfile` and `Media` in `src/profile.rs`, then add it to `ALL` |
| Same TSPL, another transparent BLE service | Set the profile's preferred service/characteristic UUIDs; use `inspect` to examine hardware |
| ZPL, CPCL or ESC/POS | Add a `LabelEncoder` implementation in `src/protocol.rs` and a `Language` variant |
| BLE credit channel or proprietary handshake | Add that protocol's connection/session handling; a writable UUID is insufficient |
| Bluetooth Classic SPP | Add a separate transport; the existing transport is BLE |
| USB interface that is not printer class 7 with bulk OUT | Extend discovery/interface selection in `src/usb.rs` |
| Windows PDF rendering | Add Windows transport qualification and packaging; macOS and Linux rendering are implemented |

## A TSPL adapter

Add the model's measured resolution and intended media to `PrinterProfile`. `Media` dimensions are in inches and may be fractional. Its dot dimensions determine raster allocation, preview output and the encoded bitmap size.

Add the profile to `profile::ALL` to register its stable CLI name and include it in `open-nelko profiles`. Give it the correct default device name and BLE channel, or set `bluetooth: None` for a USB-only printer. The existing USB transport accepts a product name, serial number or vendor/product identifier; it currently discovers standard USB printer-class interfaces.

Then verify the physical result. Similar branding or matching services do not establish that a device accepts the same bitmap polarity, orientation or print command. `--characteristic` is a diagnostic override and does not install support for a new protocol.

## Another command language

Implement `LabelEncoder::encode(&Label)` and add a `Language` variant and dispatch in `protocol::encode`. The label exposes dimensions through `media()`, copies, bytes per row and its packed bitmap. Rows run top to bottom; the most significant bit is the leftmost pixel; 1 is white and 0 is black. Padding at the end of a row is white. An encoder can invert/repack this data for a different protocol.

Set up label dimensions, image data and copy count using that printer's commands. Keep command encoding out of the USB/Bluetooth modules. The current `status` subcommand is specifically a TSPL status query; provide language-specific status handling before enabling it for a different language.

## Qualification

Use a synthetic asymmetric page with text, a barcode and marks near every edge. Check orientation, margins, full physical size and barcode readability. Test each advertised transport independently and distinguish a successful write from an observed print. Record the model/firmware and results in `HARDWARE.md`.

The only physically verified model/transport in this revision is the PL70e-BT over Bluetooth. USB and other models require their own hardware checks.
