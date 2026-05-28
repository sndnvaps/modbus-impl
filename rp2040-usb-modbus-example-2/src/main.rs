#![no_std]
#![no_main]
#![allow(dead_code)]
#![allow(unused_imports)]

use hal::{entry, pac};
use rp2040_hal as hal;

use modbus_impl::{
    BitRead, BitWrite, Coil, FrameLen4Func, Hreg, Ireg, Ists, ModbusCtx, Random, RegisterRead,
    RegisterWrite,
};
use rp_usb_serial::RpUsbConsole;

use panic_halt as _;

const XTAL_FREQ_HZ: u32 = 12_000_000u32;

const MAX_QTY: usize = 16; // 支持 01/02 的 bit quantity 与 03/04 的 register quantity 上限
const REG_COUNT: usize = 100;

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

fn u8_to_bool(value: u8) -> bool {
    value != 0
}

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    //let core = pac::CorePeripherals::take().unwrap();

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

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

    //let mut delay = Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    // 初始化 USB CDC（rp-usb-serial）
    RpUsbConsole::init(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        &mut pac.RESETS,
        clocks.usb_clock,
    );

    // Modbus 数据源
    let mut hregs: Hreg<REG_COUNT> = Hreg::new(); // FC03
    let mut iregs: Ireg<REG_COUNT> = Ireg::new(); // FC04
    let mut coils: Coil<REG_COUNT> = Coil::new(); // FC01
    let mut ists: Ists<REG_COUNT> = Ists::new(); // FC02

    // ModbusCtx：把四类资源绑到一个上下文里
    let mut ctx = ModbusCtx {
        holdings: &mut hregs,
        inputs: &mut iregs,
        coils: &mut coils,
        ists: &mut ists,
    };

    // 累积接收：请求固定 8 字节
    let mut rx_accum = [0u8; 1024];
    let mut rx_len: usize = 0;

    // 响应缓冲：足够容纳所有成功响应
    // FC03/04 最大：5 + 2*MAX_QTY
    // FC15/16 成功响应是 8 字节，也在其中
    let mut resp_buf = [0u8; 5 + MAX_QTY * 2];

    // 异常固定 5 字节
    let mut exc_buf = [0u8; 5];

    let mut tmp = [0u8; 128];

    loop {
        //更新线圈（FC01），功能测试正常
        //let val1 = Random(0, 9);
        //let val2 = Random(0, 9);
        //ctx.coils.set_bit(0, u8_to_bool(val1 as u8));
        //ctx.coils.set_bit(1, u8_to_bool(val2 as u8));

        //离散输入（FC02），功能测试正常
        //let val1 = Random(0, 2);
        //let val2 = Random(0, 1);
        //ctx.ists.set_bit(0, u8_to_bool(val1 as u8));
        //ctx.ists.set_bit(1, u8_to_bool(val2 as u8));

        // 更新保持寄存器（FC03） ,功能测试正常
        let val1 = Random(250, 350);
        let val2 = Random(330, 480);
        ctx.holdings.set(0, val1);
        ctx.holdings.set(1, val2);

        // 更新输入寄存器（FC04）,功能测试正常
        //let val1 = Random(200, 350);
        //let val2 = Random(350, 450);
        //ctx.inputs.set(0, val1);
        //ctx.inputs.set(1, val2);

        //FC05,功能测试正常

        //FC06,功能测试正常

        //FC15,功能测试正常

        //FC16,功能测试正常

        // USB 维护 + 搬运 RX 到库内部队列
        RpUsbConsole::poll();

        let n = RpUsbConsole::read(&mut tmp);
        if n > 0 {
            for &b in &tmp[..n] {
                if rx_len < rx_accum.len() {
                    rx_accum[rx_len] = b;
                    rx_len += 1;
                } else {
                    // overflow：丢弃并重新同步
                    rx_len = 0;
                }

                // 尝试解析尽可能多的帧
                loop {
                    if rx_len < 2 {
                        break;
                    }

                    let func = rx_accum[1];
                    let needed_opt = FrameLen4Func(func, &rx_accum, rx_len);

                    let needed = match needed_opt {
                        Some(l) => l,
                        None => {
                            // 关键修复：FC15/FC16 在头部未收齐（<7）时不要丢字节
                            if (func == 0x0F || func == 0x10) && rx_len < 7 {
                                break; // 等更多字节到来
                            } else {
                                // 不支持或无法确定：丢弃1字节继续找同步点
                                rx_accum.copy_within(1..rx_len, 0);
                                rx_len -= 1;
                                continue;
                            }
                        }
                    };

                    if rx_len < needed {
                        break; // 还没收满整帧，等更多字节
                    }

                    let frame = &rx_accum[..needed];
                    let resp_len = ctx.pharse_frame::<MAX_QTY>(frame, &mut resp_buf, &mut exc_buf);

                    if resp_len == 5 {
                        RpUsbConsole::write(&exc_buf[..resp_len]);
                    } else {
                        RpUsbConsole::write(&resp_buf[..resp_len]);

                        //when receive fc05 func_code
                        //ctx.coils.get(0);

                        //when receive fc06 func_code
                        //ctx.holdings.get(0);

                        //when receive FC15 func_code
                        //let qty = ctx.coils.get_qty();
                        //for i in 0..qty {
                        //    ctx.coils.get(i as u16);
                        //}

                        //when receive FC16 func_code
                        //let qty = ctx.holdings.get_qty();
                        //for i in 0..qty {
                         //   ctx.holdings.get(i as u16);
                        //}


                    }

                    // 丢弃已处理帧，保留剩余字节
                    let remaining = rx_len - needed;
                    if remaining > 0 {
                        rx_accum.copy_within(needed..rx_len, 0);
                    }
                    rx_len = remaining;
                }

                // 不要长 delay（否则USB枚举/轮询会超时）
                core::hint::spin_loop();
            }
        }
    }
}
