# Modbus-Impl (RTU) for rp-usb-serial

A small `no_std` Modbus RTU helper library designed to run on embedded Rust targets (e.g. RP2040/RP2350) and work with your `rp-usb-serial` USB CDC link. It currently focuses on **Modbus function codes 01, 02, 03, and 04** (read operations) and builds valid Modbus RTU frames including **CRC16**.

---

## What it does

At runtime the library processes fixed-length Modbus requests carried over a byte-stream transport (USB CDC). For each incoming request frame it:

1. Validates **CRC16 (Modbus polynomial 0xA001, init 0xFFFF)**  
2. Parses the request fields:
   - Unit ID
   - Function code (0x01/0x02/0x03/0x04)
   - Start address
   - Quantity
3. Checks **address range** using `is_valid(addr)`
4. Builds one of:
   - A normal response frame for the requested function, or
   - An exception response frame:
     - Function | 0x80
     - Exception code (ILLEGAL_FUNCTION / ILLEGAL_DATA_ADDRESS / ILLEGAL_DATA_VALUE)
     - CRC16

---

## Supported Modbus Functions

- **0x01** Read Coils  
- **0x02** Read Discrete Inputs  
- **0x03** Read Holding Registers (16-bit registers, big-endian in the payload)  
- **0x04** Read Input Registers (16-bit registers, big-endian in the payload)

Coils/inputs are packed into bytes using Modbus rules (**LSB-first bit packing**).

---

## Data Model

The library defines interfaces so you can plug in your own memory map:

### `RegisterRead`
Used for 16-bit register based functions (FC03/FC04):
- `get(addr: u16) -> u16`
- `is_valid(addr: u16) -> bool`

### `BitRead`
Used for bit based functions (FC01/FC02):
- `get(addr: u16) -> bool`
- `is_valid(addr: u16) -> bool`

It also provides basic storage types that implement these traits:
- `Hreg<N>` for Holding Registers (FC03)
- `Ireg<N>` for Input Registers (FC04)
- `Coil<N>` for Coils (FC01)
- `Ists<N>` for Discrete Inputs (FC02)

---

## Implementation Overview

Key components:

- **`crc16_modbus(data: &[u8]) -> u16`**  
  Implements Modbus RTU CRC16.

- **Frame parsing**  
  - Requests are assumed to be **8 bytes long** (standard RTU frame for function 01/02/03/04 read requests).
  - `parse_req03()` exists for FC03 parsing.
  - `parse_pdu()` / `PduReq` dispatch supports multiple function codes.

- **Response builders**
  - `build_resp_bit_reads()` builds FC01/FC02 responses.
  - `build_resp_regs()` builds FC03/FC04 responses.
  - `build_exception_resp()` builds exception responses.

- **`ModbusCtx::pharse_pdu()`**
  The main entry that takes a request frame and outputs either a response or an exception.

---

## Typical Usage Flow

In your main loop you typically:
1. Receive bytes from `rp-usb-serial` into an 8-byte buffer.
2. Call:
   - `ctx.pharse_pdu::<MAX_QTY>(&req8, &mut resp_buf, &mut exc_buf)`
3. Send the resulting frame back with:
   - `RpUsbConsole::write(&resp_or_exc[..len])`

---

## Notes / Limitations

- This library is RTU-focused but transport-agnostic: it assumes requests arrive as a byte stream and are accumulated into **exact 8-byte frames**.
- Only **read** functions are implemented (01/02/03/04). Write functions (06/15/16/…) are not included.
- Bit packing follows Modbus LSB-first conventions.

---