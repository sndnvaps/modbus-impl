#![no_std]
/// RegisterRead：支持 is_valid(addr) 用于越界检查
pub trait RegisterRead {
    fn get(&self, addr: u16) -> u16;
    fn is_valid(&self, addr: u16) -> bool;
}
/// 支持 01/02 的位读取（Coil/ISTS）
pub trait BitRead {
    fn get(&self, addr: u16) -> bool;
    fn is_valid(&self, addr: u16) -> bool;
}

/// Coil 结构体（FC01）
pub struct Coil<const N: usize> {
    regs: [bool; N],
}
impl<const N: usize> Coil<N> {
    pub const fn new() -> Self {
        Self { regs: [false; N] }
    }
    pub fn set_bit(&mut self, addr: u16, val: bool) {
        let i = addr as usize;
        if i < N {
            self.regs[i] = val;
        }
    }
    pub fn as_bits(&self) -> &[bool] {
        &self.regs
    }
}
impl<const N: usize> BitRead for Coil<N> {
    fn get(&self, addr: u16) -> bool {
        let i = addr as usize;
        if i < N {
            self.regs[i]
        } else {
            false
        }
    }
    fn is_valid(&self, addr: u16) -> bool {
        (addr as usize) < N
    }
}

/// ISTS（离散输入）结构体（FC02）
pub struct Ists<const N: usize> {
    regs: [bool; N],
}
impl<const N: usize> Ists<N> {
    pub const fn new() -> Self {
        Self { regs: [false; N] }
    }
    pub fn set_bit(&mut self, addr: u16, val: bool) {
        let i = addr as usize;
        if i < N {
            self.regs[i] = val;
        }
    }
}
impl<const N: usize> BitRead for Ists<N> {
    fn get(&self, addr: u16) -> bool {
        let i = addr as usize;
        if i < N {
            self.regs[i]
        } else {
            false
        }
    }
    fn is_valid(&self, addr: u16) -> bool {
        (addr as usize) < N
    }
}

/// IREG（输入寄存器）结构体（FC04）
pub struct Ireg<const N: usize> {
    regs: [u16; N],
}
impl<const N: usize> Ireg<N> {
    pub const fn new() -> Self {
        Self { regs: [0; N] }
    }
    pub fn set(&mut self, addr: u16, val: u16) {
        let i = addr as usize;
        if i < N {
            self.regs[i] = val;
        }
    }
}
impl<const N: usize> RegisterRead for Ireg<N> {
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
pub struct Hreg<const N: usize> {
    regs: [u16; N],
}

impl<const N: usize> Hreg<N> {
    pub const fn new() -> Self {
        Self { regs: [0; N] }
    }

    pub fn set(&mut self, addr: u16, val: u16) {
        let i = addr as usize;
        if i < N {
            self.regs[i] = val;
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

/// Modbus RTU CRC16，用于生成CRC16验证码
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

/// 组装 FC01 / FC02 响应：
/*
  Unit(1) + Func(1) + ByteCount(1) + PackedBits(N) + CRC(2)

  packed bits：Modbus 规定每字节 bit0 是第一个位（LSB-first）
*/
pub fn build_resp_bit_reads<const MAX_QTY: usize, B: BitRead>(
    out: &mut [u8],
    unit_id: u8,
    func: u8, // 0x01 or 0x02
    start_addr: u16,
    quantity: u16,
    bits: &B,
) -> usize {
    let qty = quantity as usize;
    let byte_cnt = (qty + 7) / 8;

    out[0] = unit_id;
    out[1] = func;
    out[2] = byte_cnt as u8;

    // 清零位区
    for i in 0..byte_cnt {
        out[3 + i] = 0;
    }

    for j in 0..qty {
        let addr = start_addr.wrapping_add(j as u16);
        let bit = bits.get(addr);
        if bit {
            let byte_i = j / 8;
            let bit_i = j % 8;
            out[3 + byte_i] |= 1u8 << bit_i; // LSB-first
        }
    }

    let body_len = 3 + byte_cnt;
    let crc = crc16_modbus(&out[..body_len]);
    out[body_len] = (crc & 0xFF) as u8;
    out[body_len + 1] = (crc >> 8) as u8;
    body_len + 2
}

/// 组装 FC04 响应（与 FC03 类似，只是 Func=0x04）
pub fn build_resp04<const MAX_QTY: usize, R: RegisterRead>(
    out: &mut [u8],
    unit_id: u8,
    start_addr: u16,
    quantity: u16,
    regs: &R,
) -> usize {
    // 复用 build_resp03，把 func 固定为 0x04
    let qty = quantity as usize;

    out[0] = unit_id;
    out[1] = 0x04;
    out[2] = (qty as u8) * 2;

    for i in 0..qty {
        let addr = start_addr.wrapping_add(i as u16);
        let v = regs.get(addr);
        let base = 3 + i * 2;
        out[base] = (v >> 8) as u8; // big-endian
        out[base + 1] = (v & 0xFF) as u8;
    }

    let body_len = 3 + qty * 2;
    let crc = crc16_modbus(&out[..body_len]);
    out[body_len] = (crc & 0xFF) as u8;
    out[body_len + 1] = (crc >> 8) as u8;
    body_len + 2
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

///ModbusCtx,用于沟通上下文
pub struct ModbusCtx<'a, H, I, C, D> {
    pub holdings: &'a mut H, // FC03
    pub inputs: &'a mut I,   // FC04
    pub coils: &'a mut C,    // FC01
    pub ists: &'a mut D,     // FC02
}

impl<'a, H, I, C, D> ModbusCtx<'a, H, I, C, D>
where
    H: RegisterRead,
    I: RegisterRead,
    C: BitRead,
    D: BitRead,
{
    /// pharse_pdu: 解析固定 8 字节 Modbus RTU 请求，并组装响应/异常
    /// 返回值：
    /// - 正常响应：返回 out_tx 中的长度
    /// - 异常响应：写入 out_exc，并返回 5
    pub fn pharse_pdu<const MAX_QTY: usize>(
        &self,
        req8: &[u8; 8],
        out_tx: &mut [u8],
        out_exc: &mut [u8; 5],
    ) -> usize {
        let unit_id = req8[0];
        let func = req8[1];

        // -------- CRC check --------
        let expected_crc = u16::from_le_bytes([req8[6], req8[7]]);
        let calc_crc = crc16_modbus(&req8[..6]);
        if expected_crc != calc_crc {
            // CRC 错：返回 ILLEGAL_DATA_VALUE，功能码用请求 func|0x80
            return build_exception_resp_fixed::<5>(
                out_exc,
                unit_id,
                func | 0x80,
                exc::ILLEGAL_DATA_VALUE,
            );
        }

        // -------- common fields --------
        let start_addr = u16::from_be_bytes([req8[2], req8[3]]);
        let quantity = u16::from_be_bytes([req8[4], req8[5]]);

        // quantity 校验
        if quantity == 0 || (quantity as usize) > MAX_QTY {
            return build_exception_resp_fixed::<5>(
                out_exc,
                unit_id,
                func | 0x80,
                exc::ILLEGAL_DATA_VALUE,
            );
        }

        match func {
            0x01 => {
                // FC01 Read Coils
                let end = start_addr.wrapping_add(quantity.saturating_sub(1));
                if !self.coils.is_valid(start_addr) || !self.coils.is_valid(end) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }
                build_resp_bit_reads::<MAX_QTY, _>(
                    out_tx, unit_id, 0x01, start_addr, quantity, self.coils,
                )
            }

            0x02 => {
                // FC02 Read Discrete Inputs
                let end = start_addr.wrapping_add(quantity.saturating_sub(1));
                if !self.ists.is_valid(start_addr) || !self.ists.is_valid(end) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }
                build_resp_bit_reads::<MAX_QTY, _>(
                    out_tx, unit_id, 0x02, start_addr, quantity, self.ists,
                )
            }

            0x03 => {
                // FC03 Read Holding Registers
                let end = start_addr.wrapping_add(quantity.saturating_sub(1));
                if !self.holdings.is_valid(start_addr) || !self.holdings.is_valid(end) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }
                build_resp_regs::<MAX_QTY, _>(
                    out_tx,
                    unit_id,
                    0x03,
                    start_addr,
                    quantity,
                    self.holdings,
                )
            }

            0x04 => {
                // FC04 Read Input Registers
                let end = start_addr.wrapping_add(quantity.saturating_sub(1));
                if !self.inputs.is_valid(start_addr) || !self.inputs.is_valid(end) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }
                build_resp_regs::<MAX_QTY, _>(
                    out_tx,
                    unit_id,
                    0x04,
                    start_addr,
                    quantity,
                    self.inputs,
                )
            }

            _ => {
                // 不支持功能码
                build_exception_resp_fixed::<5>(
                    out_exc,
                    unit_id,
                    func | 0x80,
                    exc::ILLEGAL_FUNCTION,
                )
            }
        }
    }
}

// ---------------- helper：异常帧（固定 5 字节） ----------------
fn build_exception_resp_fixed<const BUF: usize>(
    out_exc: &mut [u8; 5],
    unit_id: u8,
    function_exception: u8,
    exception_code: u8,
) -> usize {
    out_exc[0] = unit_id;
    out_exc[1] = function_exception;
    out_exc[2] = exception_code;

    let crc = crc16_modbus(&out_exc[..3]);
    out_exc[3] = (crc & 0xFF) as u8;
    out_exc[4] = (crc >> 8) as u8;
    5
}
// ---------------- helper：reg 读取响应（FC03/FC04） ----------------
// out_tx 需要至少 >= 3 + MAX_QTY*2 + 2
fn build_resp_regs<const MAX_QTY: usize, R: RegisterRead>(
    out_tx: &mut [u8],
    unit_id: u8,
    func: u8,
    start_addr: u16,
    quantity: u16,
    regs: &R,
) -> usize {
    let qty = quantity as usize;

    out_tx[0] = unit_id;
    out_tx[1] = func;
    out_tx[2] = (qty as u8) * 2;

    for i in 0..qty {
        let addr = start_addr.wrapping_add(i as u16);
        let v = regs.get(addr);
        let base = 3 + i * 2;
        out_tx[base] = (v >> 8) as u8;
        out_tx[base + 1] = (v & 0xFF) as u8;
    }

    let body_len = 3 + qty * 2;
    let crc = crc16_modbus(&out_tx[..body_len]);
    out_tx[body_len] = (crc & 0xFF) as u8;
    out_tx[body_len + 1] = (crc >> 8) as u8;

    body_len + 2
}
