#![no_std]

/// RegisterRead：支持 is_valid(addr) 用于越界检查
pub trait RegisterRead {
    fn get(&self, addr: u16) -> u16;
    fn is_valid(&self, addr: u16) -> bool;
}

pub struct Hreg<const N: usize> {
    regs: [u16; N],
}

impl<const N: usize> Hreg<N> {
    pub const fn new() -> Self {
        Self { regs: [0; N] }
    }
    /// 写入前两个寄存器：regs[0]=val1, regs[1]=val2
    pub fn set(&mut self, val1: u16, val2: u16) {
        // N==0 时数组不存在（虽然 const 泛型一般不会用 N=0，但做个保护）
        if N >= 1 {
            self.regs[0] = val1;
        }
        if N >= 2 {
            self.regs[1] = val2;
        }
    }

    pub fn as_slice(&self) -> &[u16] {
        &self.regs
    }
}

// RegisterRead 实现：提供 get + is_valid
impl<const N: usize> RegisterRead for Hreg<N> {
    fn get(&self, addr: u16) -> u16 {
        let i = addr as usize;
        if i < N {
            self.regs[i]
        } else {
            0
        }
    }
    fn is_valid(&self, addr: u16) -> bool {
        (addr as usize) < N
    }
}

///////////////////////////////////////////////////////////////////////////////
// 2) random(start_val, end_val)  —— no_std 伪随机
///////////////////////////////////////////////////////////////////////////////

// xorshift32 seed：单线程场景足够用
static mut SEED: u32 = 0x1234_5678;

#[inline]
fn xorshift32_next() -> u32 {
    // SAFETY: 单线程典型嵌入式用法；如需多核/中断并发请告诉我再改为更安全的方案
    unsafe {
        let mut x = SEED;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        SEED = x;
        x
    }
}

/// 生成闭区间 [min(start_val,end_val), max(...)] 内的随机 u16
pub fn random(start_val: u16, end_val: u16) -> u16 {
    let (lo, hi) = if start_val <= end_val {
        (start_val, end_val)
    } else {
        (end_val, start_val)
    };

    let span = (hi as u32).wrapping_sub(lo as u32).wrapping_add(1);
    let r = xorshift32_next() % span;
    (lo as u32 + r) as u16
}

#[derive(Clone, Copy, Debug)]
pub struct Req03 {
    pub unit_id: u8,
    pub start_addr: u16,
    pub quantity: u16,
}

pub mod exc {
    pub const ILLEGAL_FUNCTION: u8 = 0x01;
    pub const ILLEGAL_DATA_ADDRESS: u8 = 0x02;
    pub const ILLEGAL_DATA_VALUE: u8 = 0x03;
}

/// Modbus RTU CRC16
pub fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            if (crc & 0x0001) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

/// 解析固定 8 字节 Modbus RTU 03 请求
pub fn parse_req03(frame: &[u8; 8]) -> Option<Req03> {
    if frame[1] != 0x03 {
        return None;
    }
    let expected_crc = u16::from_le_bytes([frame[6], frame[7]]);
    let calc_crc = crc16_modbus(&frame[..6]);
    if expected_crc != calc_crc {
        return None;
    }

    let start_addr = u16::from_be_bytes([frame[2], frame[3]]);
    let quantity = u16::from_be_bytes([frame[4], frame[5]]);

    Some(Req03 {
        unit_id: frame[0],
        start_addr,
        quantity,
    })
}

/// 组装 03 响应帧：Unit(1)+Func(1)+ByteCount(1)+Reg(2*quantity)+CRC(2)
pub fn build_resp03<const MAX_QTY: usize, R: RegisterRead>(
    out: &mut [u8],
    unit_id: u8,
    start_addr: u16,
    quantity: u16,
    regs: &R,
) -> usize {
    let qty = quantity as usize;
    // 要求：quantity <= MAX_QTY，且 out 足够大
    out[0] = unit_id;
    out[1] = 0x03;
    out[2] = (qty as u8) * 2;

    for i in 0..qty {
        let addr = start_addr.wrapping_add(i as u16);
        let v = regs.get(addr);
        let base = 3 + i * 2;
        out[base] = (v >> 8) as u8; // register big-endian
        out[base + 1] = (v & 0xFF) as u8;
    }

    let body_len = 3 + qty * 2;
    let crc = crc16_modbus(&out[..body_len]);
    out[body_len] = (crc & 0xFF) as u8; // CRC low
    out[body_len + 1] = (crc >> 8) as u8; // CRC high
    body_len + 2
}

/// 组装异常响应：Function=03|0x80 + ExceptionCode(1) + CRC(2)
pub fn build_exception_resp<const BUF: usize>(
    out: &mut [u8; BUF],
    unit_id: u8,
    function_exception: u8, // 例如 0x83
    exception_code: u8,
) -> usize {
    out[0] = unit_id;
    out[1] = function_exception;
    out[2] = exception_code;

    let crc = crc16_modbus(&out[..3]);
    out[3] = (crc & 0xFF) as u8;
    out[4] = (crc >> 8) as u8;
    5
}

/// phrase_pdu：处理 Modbus PDU（目前实现 03）
pub fn phrase_pdu<const MAX_QTY: usize, R: RegisterRead>(
    req8: &[u8; 8],
    regs: &R,
    out_tx: &mut [u8], // 至少 5 + MAX_QTY*2
    out_exc: &mut [u8; 5],
) -> usize {
    let unit_id = req8[0];
    let func = req8[1];

    match func {
        0x03 => {
            let Some(r) = parse_req03(req8) else {
                return build_exception_resp::<5>(out_exc, unit_id, 0x83, exc::ILLEGAL_DATA_VALUE);
            };

            // quantity 校验
            if r.quantity == 0 || (r.quantity as usize) > MAX_QTY {
                return build_exception_resp::<5>(
                    out_exc,
                    r.unit_id,
                    0x83,
                    exc::ILLEGAL_DATA_VALUE,
                );
            }

            // start..start+quantity-1 越界校验
            let start_u32 = r.start_addr as u32;
            let qty_u32 = r.quantity as u32;

            let end_u32 = start_u32.saturating_add(qty_u32 - 1);
            if end_u32 > 0xFFFF {
                return build_exception_resp::<5>(
                    out_exc,
                    r.unit_id,
                    0x83,
                    exc::ILLEGAL_DATA_ADDRESS,
                );
            }
            let end_addr = end_u32 as u16;

            if !regs.is_valid(r.start_addr) || !regs.is_valid(end_addr) {
                return build_exception_resp::<5>(
                    out_exc,
                    r.unit_id,
                    0x83,
                    exc::ILLEGAL_DATA_ADDRESS,
                );
            }

            build_resp03::<MAX_QTY, _>(out_tx, r.unit_id, r.start_addr, r.quantity, regs)
        }

        _ => build_exception_resp::<5>(out_exc, unit_id, 0x83, exc::ILLEGAL_FUNCTION),
    }
}
