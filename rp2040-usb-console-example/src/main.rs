#![no_std]
#![no_main]
// For string formatting.
// The macro for our start-up function
// A shorter alias for the Peripheral Access Crate, which provides low-level
// register access
use hal::{entry, pac, Clock};
use rp2040_hal as hal;

use modbus_impl::{phrase_pdu, random, Hreg};
use rp_usb_serial::RpUsbConsole;

// Ensure we halt the program on panic (if we don't mention this crate it won't
// be linked)
use panic_halt as _;

/// External high-speed crystal on the Raspberry Pi Pico board is 12 MHz. Adjust
/// if your board has a different frequency
const XTAL_FREQ_HZ: u32 = 12_000_000u32;

/// The linker will place this boot block at the start of our program image. We
/// need this to help the ROM bootloader get our code up and running.
/// Note: This boot block is not necessary when using a rp-hal based BSP
/// as the BSPs already perform this step.
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

/// Entry point to our bare-metal application.
///
/// The `#[entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables are initialised.
///
/// The function configures the RP2040 peripherals,
/// gets a handle on the I2C peripheral,
/// initializes the SSD1306 driver, initializes the text builder
/// and then draws some text on the display.
///
///

const MAX_QTY: usize = 16; // 响应最大读取寄存器数（<=16）

// 模拟 Holding Registers
const REG_COUNT: usize = 100;

#[entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    //
    // The default is to generate a 125 MHz system clock
    let clocks = hal::clocks::init_clocks_and_plls(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());
    // The single-cycle I/O block controls our GPIO pins
    // let sio = hal::Sio::new(pac.SIO);

    // 初始化 USB
    RpUsbConsole::init(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        &mut pac.RESETS,
        clocks.usb_clock,
    );

    // 接收累积：Modbus 03 请求固定 8 字节
    let mut rx_accum = [0u8; 64];
    let mut rx_len: usize = 0;

    // 响应缓冲：最大 03 响应长度 = 1+1+1+2*MAX_QTY +2(CRC)
    // = 5 + 2*MAX_QTY
    let mut resp_buf = [0u8; 5 + MAX_QTY * 2];
    let mut exc_buf = [0u8; 5];

    // 临时读缓冲（从 usb rx_buf 取字节可能不止 1 次）
    let mut tmp = [0u8; 64];

    let mut hregs: Hreg<REG_COUNT> = Hreg::new();

    loop {
        // 生成两个随机值：写入寄存器0/1
        let val1 = random(250, 350);
        let val2 = random(250, 350);

        hregs.set(val1, val2);

        // 维护 USB + 搬运 RX 到内部队列
        RpUsbConsole::poll();

        let n = RpUsbConsole::read(&mut tmp);
        if n == 0 {
            continue;
        }

        for &b in &tmp[..n] {
            if rx_len < rx_accum.len() {
                rx_accum[rx_len] = b;
                rx_len += 1;
            } else {
                // overflow 简单策略：丢弃并重新同步
                rx_len = 0;
            }

            // 若累计够 8 字节，则解析并响应（支持粘包）
            while rx_len >= 8 {
                let mut req8 = [0u8; 8];
                req8.copy_from_slice(&rx_accum[..8]);

                let resp_len = phrase_pdu::<MAX_QTY, _>(&req8, &hregs, &mut resp_buf, &mut exc_buf);

                // 约定：异常长度固定 5；正常 resp_len !=5
                if resp_len == 5 {
                    RpUsbConsole::write(&exc_buf[..resp_len]);
                } else {
                    RpUsbConsole::write(&resp_buf[..resp_len]);
                }

                // 左移 rx_accum，把剩余字节保留
                let remaining = rx_len - 8;
                if remaining > 0 {
                    rx_accum.copy_within(8..rx_len, 0);
                }
                rx_len = remaining;
            }
        }
        delay.delay_ms(1500);
    }
}
