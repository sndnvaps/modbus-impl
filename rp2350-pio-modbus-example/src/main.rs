#![no_std]
#![no_main]

//gpio9 as RX pin connect to ch340-uart-mod's TX pin
//gpio10 as TX pin connect to ch340-uart-mod's RX pin
//baud is 115200
// UartConfig is 8N1

// For string formatting.
// The macro for our start-up function
// A shorter alias for the Peripheral Access Crate, which provides low-level
// register access
use rp235x_hal as hal;

use hal::{
    clocks::init_clocks_and_plls,
    entry,
    gpio::{FunctionPio0, Pins},
    pac,
    pac::PIO0,
    pio::PIOExt,
    pio::SM0,
    pio::SM1,
    sio::Sio,
    watchdog::Watchdog,
    Clock,
};

use rp_pio_serial::{DataBits, Parity, RpPioSerial, ServiceMode, StopBits, UartConfig};

use modbus_impl::{random, BitRead, Coil, Hreg, Ireg, Ists, ModbusCtx, RegisterRead};

// Ensure we halt the program on panic (if we don't mention this crate it won't
// be linked)
use panic_halt as _;

/// External high-speed crystal on the Raspberry Pi Pico board is 12 MHz. Adjust
/// if your board has a different frequency
const XTAL_FREQ_HZ: u32 = 12_000_000u32;

const MAX_QTY: usize = 16; // 支持 01/02 的 bit quantity 与 03/04 的 register quantity 上限
const REG_COUNT: usize = 100;

/// The linker will place this boot block at the start of our program image. We
/// need this to help the ROM bootloader get our code up and running.
/// Note: This boot block is not necessary when using a rp-hal based BSP
/// as the BSPs already perform this step.
#[link_section = ".start_block"]
#[used]
pub static IMAGE_DEF: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

fn u8_to_bool(value: u8) -> bool {
    value != 0
}

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

#[entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();
    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    //
    // The default is to generate a 125 MHz system clock
    let clocks = init_clocks_and_plls(
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

    //let mut delay = cortex_m::delay::Delay::new(core.PLL_SYS, clocks.system_clock.freq().to_Hz());
    //let mut timer = hal::Timer::new_timer0(pac.TIMER0, &mut pac.RESETS, &clocks);
    // The single-cycle I/O block controls our GPIO pins
    let sio = Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // PIO0 -> FunctionPio0
    let _tx_pin = pins.gpio10.into_function::<FunctionPio0>();
    let _rx_pin = pins.gpio9.into_function::<FunctionPio0>();

    // 演示“任意状态机组合”：这里使用 SM2 / SM3
    let (mut pio0, sm0, sm1, _sm2, _sm3) = pac.PIO0.split(&mut pac.RESETS);

    let config = UartConfig {
        baud: 115_200u32,
        data_bits: DataBits::Eight,
        parity: Parity::None,
        stop_bits: StopBits::One,
        auto_echo: false,
        service_mode: ServiceMode::Polling,
        tx_pipeline_chars: 5,
    };

    let mut serial: RpPioSerial<PIO0, SM0, SM1, 512, 512> = RpPioSerial::new(
        &mut pio0,
        sm0, // TX SM
        sm1, // RX SM
        10,  // TX pin
        9,   // RX pin
        clocks.system_clock.freq().to_Hz(),
        config,
    )
    .unwrap();

    // Modbus 数据源
    let mut hregs: Hreg<REG_COUNT> = Hreg::new(); // FC03
    let mut iregs: Ireg<REG_COUNT> = Ireg::new(); // FC04
    let mut coils: Coil<REG_COUNT> = Coil::new(); // FC01
    let mut ists: Ists<REG_COUNT> = Ists::new(); // FC02

    // ModbusCtx：把四类资源绑到一个上下文里
    let ctx = ModbusCtx {
        holdings: &mut hregs,
        inputs: &mut iregs,
        coils: &mut coils,
        ists: &mut ists,
    };

    // 累积接收：请求固定 8 字节
    let mut rx_accum = [0u8; 64];
    let mut rx_len: usize = 0;

    // 响应缓冲：
    // FC03/04 最大长度 = 1+1+1+2*MAX_QTY+2 = 5 + 2*MAX_QTY
    let mut resp_buf = [0u8; 5 + MAX_QTY * 2];

    // 异常固定 5 字节
    let mut exc_buf = [0u8; 5];

    //接收请求信息
    let mut rx_buf = [0u8; 64];

    loop {
        //更新线圈（FC01），功能测试正常
        //let val1 = random(0, 1);
        //let val2 = random(0, 3);
        //ctx.coils.set_bit(0, u8_to_bool(val1 as u8));
        //ctx.coils.set_bit(1, u8_to_bool(val2 as u8));

        //离散输入（FC02），功能测试正常
        //let val1 = random(0, 2);
        //let val2 = random(0, 1);
        //ctx.ists.set_bit(0, u8_to_bool(val1 as u8));
        //ctx.ists.set_bit(1, u8_to_bool(val2 as u8));

        // 更新保持寄存器（FC03） ,功能测试正常
        let val1 = random(250, 350);
        let val2 = random(330, 480);
        ctx.holdings.set(0, val1);
        ctx.holdings.set(1, val2);

        // 更新输入寄存器（FC04）,功能测试正常
        //let val1 = random(200, 350);
        //let val2 = random(350, 450);
        //ctx.inputs.set(0, val1);
        //ctx.inputs.set(1, val2);

        serial.poll();

        let n = serial.read(&mut rx_buf);
        if n > 0 {
            for &b in &rx_buf[..n] {
                if rx_len < rx_accum.len() {
                    rx_accum[rx_len] = b;
                    rx_len += 1;
                } else {
                    // overflow：丢弃并重新同步
                    rx_len = 0;
                }

                // 每凑够 8 字节解析一次（支持粘包）
                while rx_len >= 8 {
                    let mut req8 = [0u8; 8];
                    req8.copy_from_slice(&rx_accum[..8]);

                    // ===== 处理响应请求ctx.pharse_pdu =====
                    let resp_len = ctx.pharse_pdu::<MAX_QTY>(&req8, &mut resp_buf, &mut exc_buf);

                    // resp_len==5 表示异常响应
                    if resp_len == 5 {
                        serial.write(&exc_buf[..resp_len]);
                    } else {
                        serial.write(&resp_buf[..resp_len]);
                    }

                    // 保留剩余字节
                    let remaining = rx_len - 8;
                    if remaining > 0 {
                        rx_accum.copy_within(8..rx_len, 0);
                    }
                    rx_len = remaining;
                }
            }
        }
    }
}
