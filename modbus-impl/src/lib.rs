#![no_std]
#![allow(non_snake_case)]
/// RegisterRead：支持 is_valid(addr) 用于越界检查
/// 支持03/04寄存器的读取(Hreg/Ireg)
pub trait RegisterRead {
    fn get(&self, addr: u16) -> u16;
    fn is_valid(&self, addr: u16) -> bool;
}

/// 用于 FC06 写单保持寄存器
pub trait RegisterWrite {
    fn set_reg(&mut self, addr: u16, val: u16);
    fn set_qty(&mut self, qty: u16);
    fn get_qty(&mut self) -> usize;
}

/// 支持 01/02 的位读取（Coil/ISTS）
pub trait BitRead {
    fn get(&self, addr: u16) -> bool;
    fn is_valid(&self, addr: u16) -> bool;
}

/// 用于 FC05 写单线圈
pub trait BitWrite {
    fn set_bit(&mut self, addr: u16, val: bool);
    fn set_qty(&mut self, qty: u16);
    fn get_qty(&mut self) -> usize;
}

/// Coil 结构体（FC01）
pub struct Coil<const N: usize> {
    regs: [bool; N],
    qty: usize,
}
impl<const N: usize> Coil<N> {
    pub const fn new() -> Self {
        Self {
            regs: [false; N],
            qty: 0,
        }
    }

    pub fn set_bit(&mut self, addr: u16, val: bool) {
        let i = addr as usize;
        if i < N {
            self.regs[i] = val;
        }
    }

    pub fn set_qty(&mut self, qty: u16) {
        let i = qty as usize;
        if i < N {
            self.qty = i;
        }
        //if i > N, qty = N-1
        self.qty = N - 1;
    }

    pub fn get_qty(&mut self) -> usize {
        self.qty
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

impl<const N: usize> BitWrite for Coil<N> {
    fn set_bit(&mut self, addr: u16, val: bool) {
        Coil::set_bit(self, addr, val);
    }

    fn set_qty(&mut self, qty: u16) {
        Coil::set_qty(self, qty);
    }

    fn get_qty(&mut self) -> usize {
        Coil::get_qty(self)
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

///HREG（保持寄存器）结构体（FC03）
pub struct Hreg<const N: usize> {
    regs: [u16; N],
    qty: usize,
}

impl<const N: usize> Hreg<N> {
    pub const fn new() -> Self {
        Self {
            regs: [0; N],
            qty: 0,
        }
    }

    pub fn set(&mut self, addr: u16, val: u16) {
        let i = addr as usize;
        if i < N {
            self.regs[i] = val;
        }
    }

    pub fn set_qty(&mut self, qty: u16) {
        let i = qty as usize;
        if i < N {
            self.qty = i;
        }
        //if i > N, qty = N-1
        self.qty = N - 1;
    }

    pub fn get_qty(&mut self) -> usize {
        self.qty
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

impl<const N: usize> RegisterWrite for Hreg<N> {
    fn set_reg(&mut self, addr: u16, val: u16) {
        Hreg::set(self, addr, val);
    }
    fn set_qty(&mut self, qty: u16) {
        Hreg::set_qty(self, qty);
    }
    fn get_qty(&mut self) -> usize {
        Hreg::get_qty(self)
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
pub fn Random(start_val: u16, end_val: u16) -> u16 {
    let (lo, hi) = if start_val <= end_val {
        (start_val, end_val)
    } else {
        (end_val, start_val)
    };

    let span = (hi as u32).wrapping_sub(lo as u32).wrapping_add(1);
    let r = xorshift32_next() % span;
    (lo as u32 + r) as u16
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
    H: RegisterRead + RegisterWrite,
    I: RegisterRead,
    C: BitRead + BitWrite,
    D: BitRead,
{
    /// pharse_pdu: 解析固定 8 字节 Modbus RTU 请求，并组装响应/异常
    /// 返回值：
    /// - 正常响应：返回 out_tx 中的长度
    /// - 异常响应：写入 out_exc，并返回 5
    pub fn pharse_pdu<const MAX_QTY: usize>(
        &mut self,
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

        match func {
            0x01 => {
                // quantity 校验
                if quantity == 0 || (quantity as usize) > MAX_QTY {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }
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
                // quantity 校验
                if quantity == 0 || (quantity as usize) > MAX_QTY {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }
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
                // quantity 校验
                if quantity == 0 || (quantity as usize) > MAX_QTY {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }
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
                // quantity 校验
                if quantity == 0 || (quantity as usize) > MAX_QTY {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }
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

            0x05 => {
                // FC05: addr=start_addr, value=quantity(0xFF00/0x0000)
                let coil_value = quantity;

                let bit = match coil_value {
                    0xFF00 => true,
                    0x0000 => false,
                    _ => {
                        return build_exception_resp_fixed::<5>(
                            out_exc,
                            unit_id,
                            0x05 | 0x80,
                            exc::ILLEGAL_DATA_VALUE,
                        );
                    }
                };

                if !self.coils.is_valid(start_addr) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        0x05 | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }

                self.coils.set_bit(start_addr, bit);

                // 正常响应：回显请求前6字节 + CRC
                out_tx[0] = unit_id;
                out_tx[1] = 0x05;
                out_tx[2] = (start_addr >> 8) as u8;
                out_tx[3] = (start_addr & 0xFF) as u8;
                out_tx[4] = (coil_value >> 8) as u8;
                out_tx[5] = (coil_value & 0xFF) as u8;

                let crc = crc16_modbus(&out_tx[..6]);
                out_tx[6] = (crc & 0xFF) as u8;
                out_tx[7] = (crc >> 8) as u8;
                8
            }

            0x06 => {
                // FC06: 写单保持寄存器：addr=start_addr, value=quantity(任意 u16)
                let reg_value = quantity;

                if !self.holdings.is_valid(start_addr) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        0x06 | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }

                self.holdings.set_reg(start_addr, reg_value);

                // 正常响应：回显请求前6字节 + CRC
                out_tx[0] = unit_id;
                out_tx[1] = 0x06;
                out_tx[2] = (start_addr >> 8) as u8;
                out_tx[3] = (start_addr & 0xFF) as u8;
                out_tx[4] = (reg_value >> 8) as u8;
                out_tx[5] = (reg_value & 0xFF) as u8;

                let crc = crc16_modbus(&out_tx[..6]);
                out_tx[6] = (crc & 0xFF) as u8;
                out_tx[7] = (crc >> 8) as u8;
                8
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

    /// pharse_frame：处理可变长度 Modbus RTU 请求帧（01/02/03/04/05/06 固定8，15/16 可变）
    /// out_tx：成功响应写入缓冲
    /// out_exc：异常响应固定 5 字节缓冲
    pub fn pharse_frame<const MAX_QTY: usize>(
        &mut self,
        req: &[u8],
        out_tx: &mut [u8],
        out_exc: &mut [u8; 5],
    ) -> usize {
        // 最小长度至少要有 unit(1)+func(1)+CRC(2)=4，但这里按 Modbus RTU 解析从 6 开始也行
        if req.len() < 8 {
            return build_exception_resp_fixed::<5>(
                out_exc,
                req.get(0).copied().unwrap_or(0),
                0x80, // func未知时用 0x80占位（也可改成0）
                exc::ILLEGAL_DATA_VALUE,
            );
        }

        let unit_id = req[0];
        let func = req[1];

        // CRC check：最后两个字节为 CRC
        let expected_crc = u16::from_le_bytes([req[req.len() - 2], req[req.len() - 1]]);
        let calc_crc = crc16_modbus(&req[..req.len() - 2]);
        if expected_crc != calc_crc {
            return build_exception_resp_fixed::<5>(
                out_exc,
                unit_id,
                func | 0x80,
                exc::ILLEGAL_DATA_VALUE,
            );
        }

        // -------- 共用字段起点：start_addr 在偏移2..4，quantity 在偏移4..6 --------
        let start_addr = u16::from_be_bytes([req[2], req[3]]);
        let quantity = u16::from_be_bytes([req[4], req[5]]);

        // -------- 处理各功能码 --------
        match func {
            0x01 | 0x02 | 0x03 | 0x04 | 0x05 | 0x06 => {
                if req.len() != 8 {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }
                let req8: [u8; 8] = req[..8].try_into().unwrap();
                return self.pharse_pdu::<MAX_QTY>(&req8, out_tx, out_exc);
            }

            0x0F => {
                // FC15：Write Multiple Coils
                // 请求结构：Unit(1) Func(1) Start(2) Quantity(2) ByteCount(1) CoilsBytes(N) CRC(2)
                // 长度：9 + ByteCount
                if req.len() < 9 {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                let byte_count = req[6] as usize;
                let expected_len = 9 + byte_count;
                if req.len() != expected_len {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                // quantity 校验（01/02 的 MAX_QTY 你用同一个常量也可以）
                if quantity == 0 || (quantity as usize) > MAX_QTY {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                // 地址范围校验：start..start+quantity-1
                let end_addr = start_addr.wrapping_add(quantity.saturating_sub(1));
                if !self.coils.is_valid(start_addr) || !self.coils.is_valid(end_addr) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }

                // ByteCount 必须等于 ceil(quantity/8)
                let min_byte_cnt = (quantity as usize + 7) / 8;
                if byte_count != min_byte_cnt {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                //set the qty for the coils
                self.coils.set_qty(quantity);

                let coils_bytes = &req[7..7 + byte_count];

                // 写入线圈（LSB-first packing）
                for j in 0..quantity as usize {
                    let addr = start_addr.wrapping_add(j as u16);
                    let byte_i = j / 8;
                    let bit_i = j % 8;
                    let bit = ((coils_bytes[byte_i] >> bit_i) & 0x01) != 0;
                    self.coils.set_bit(addr, bit);
                }

                // 响应：echo Unit Func StartAddr Quantity CRC（固定8字节）
                // out_tx 需要至少 8 字节
                out_tx[0] = unit_id;
                out_tx[1] = 0x0F;
                out_tx[2] = (start_addr >> 8) as u8;
                out_tx[3] = (start_addr & 0xFF) as u8;
                out_tx[4] = (quantity >> 8) as u8;
                out_tx[5] = (quantity & 0xFF) as u8;

                let crc = crc16_modbus(&out_tx[..6]);
                out_tx[6] = (crc & 0xFF) as u8;
                out_tx[7] = (crc >> 8) as u8;
                8
            }

            0x10 => {
                // FC16：Write Multiple Registers
                // 请求结构：Unit Func Start(2) Quantity(2) ByteCount(1) RegBytes(2*Quantity) CRC(2)
                if req.len() < 9 {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                let byte_count = req[6] as usize;
                let expected_len = 9 + byte_count;
                if req.len() != expected_len {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                if quantity == 0 || (quantity as usize) > MAX_QTY {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                let end_addr = start_addr.wrapping_add(quantity.saturating_sub(1));
                if !self.holdings.is_valid(start_addr) || !self.holdings.is_valid(end_addr) {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_ADDRESS,
                    );
                }

                // ByteCount 必须等于 quantity*2
                if byte_count != (quantity as usize) * 2 {
                    return build_exception_resp_fixed::<5>(
                        out_exc,
                        unit_id,
                        func | 0x80,
                        exc::ILLEGAL_DATA_VALUE,
                    );
                }

                //写入qty数据，方便后面调用
                self.holdings.set_qty(quantity);

                let reg_bytes = &req[7..7 + byte_count];

                //写入保存寄存器
                for j in 0..quantity as usize {
                    let addr = start_addr.wrapping_add(j as u16);
                    let base = j * 2;
                    let v = u16::from_be_bytes([reg_bytes[base], reg_bytes[base + 1]]);
                    self.holdings.set_reg(addr, v);
                }

                // 响应：echo Unit Func StartAddr Quantity CRC（固定8字节）
                out_tx[0] = unit_id;
                out_tx[1] = 0x10;
                out_tx[2] = (start_addr >> 8) as u8;
                out_tx[3] = (start_addr & 0xFF) as u8;
                out_tx[4] = (quantity >> 8) as u8;
                out_tx[5] = (quantity & 0xFF) as u8;

                let crc = crc16_modbus(&out_tx[..6]);
                out_tx[6] = (crc & 0xFF) as u8;
                out_tx[7] = (crc >> 8) as u8;
                8
            }

            _ => build_exception_resp_fixed::<5>(
                out_exc,
                unit_id,
                func | 0x80,
                exc::ILLEGAL_FUNCTION,
            ),
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

/// FrameLen4Func,获取func需要用到的数据长度
/// get the len for func_code
#[inline]
pub fn FrameLen4Func(func: u8, buf: &[u8], cur_len: usize) -> Option<usize> {
    // 固定 8 字节功能码
    match func {
        0x01 | 0x02 | 0x03 | 0x04 | 0x05 | 0x06 => return Some(8),

        // FC15/FC16：长度 = 9 + ByteCount
        0x0F | 0x10 => {
            if cur_len < 7 {
                return None; // 还没拿到 ByteCount
            }
            let byte_count = buf[6] as usize;
            return Some(9 + byte_count);
        }
        _ => return None,
    }
}
