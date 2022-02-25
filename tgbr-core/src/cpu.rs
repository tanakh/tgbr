use bitvec::prelude::*;
use log::{debug, log_enabled, trace, Level};
use serde::{Deserialize, Serialize};

use crate::{
    context,
    util::{trait_alias, ConstEval},
};

#[derive(Default, Serialize, Deserialize)]
pub struct Cpu {
    halting: bool,
    interrupt_master_enable: bool,
    prev_interrupt_enable: bool,
    reg: Register,
    cycle: u64,
    period: u64,
}

trait_alias!(pub trait Context = context::Bus + context::InterruptFlag);

#[derive(Default, Serialize, Deserialize)]
pub struct Register {
    pub a: u8,
    pub f: Flag,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Register {
    fn af(&self) -> u16 {
        ((self.a as u16) << 8) | (self.f.pack() as u16)
    }

    fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f.unpack(value as u8);
    }

    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }

    fn set_bc(&mut self, data: u16) {
        self.b = (data >> 8) as u8;
        self.c = (data & 0xFF) as u8;
    }

    fn de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    fn set_de(&mut self, data: u16) {
        self.d = (data >> 8) as u8;
        self.e = (data & 0xFF) as u8;
    }

    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | (self.l as u16)
    }

    fn set_hl(&mut self, data: u16) {
        self.h = (data >> 8) as u8;
        self.l = (data & 0xFF) as u8;
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Flag {
    pub z: bool,
    pub n: bool,
    pub h: bool,
    pub c: bool,
}

impl Flag {
    pub fn pack(&self) -> u8 {
        let mut data = 0;
        let v = data.view_bits_mut::<Lsb0>();
        v.set(7, self.z);
        v.set(6, self.n);
        v.set(5, self.h);
        v.set(4, self.c);
        data
    }

    pub fn unpack(&mut self, data: u8) {
        let v = data.view_bits::<Lsb0>();
        self.z = v[7];
        self.n = v[6];
        self.h = v[5];
        self.c = v[4];
    }
}

#[rustfmt::skip]
macro_rules! instructions {
    ($cont:ident) => { indexing! { $cont @start:
        //       0 / 8       1 / 9      2 / A       3 / B      4 / C       5 / D      6 / E       7 / F
        /* 00 */ NOP;        LD BC,nn;  LD (BC),A;  INC BC;    INC B;      DEC B;     LD B,n;     RLCA;
        /* 08 */ LD (nn),SP; ADD HL,BC; LD A,(BC);  DEC BC;    INC C;      DEC C;     LD C,n;     RRCA;
        /* 10 */ STOP;       LD DE,nn;  LD (DE),A;  INC DE;    INC D;      DEC D;     LD D,n;     RLA;
        /* 18 */ JR r8;      ADD HL,DE; LD A,(DE);  DEC DE;    INC E;      DEC E;     LD E,n;     RRA;
        /* 20 */ JR NZ,r8;   LD HL,nn;  LD (^HL),A; INC HL;    INC H;      DEC H;     LD H,n;     DAA;
        /* 28 */ JR Z,r8;    ADD HL,HL; LD A,(^HL); DEC HL;    INC L;      DEC L;     LD L,n;     CPL;
        /* 30 */ JR NC,r8;   LD SP,nn;  LD (-HL),A; INC SP;    INC (HL);   DEC (HL);  LD (HL),n;  SCF;
        /* 38 */ JR C,r8;    ADD HL,SP; LD A,(-HL); DEC SP;    INC A;      DEC A;     LD A,n;     CCF;
        /* 40 */ LD B,B;     LD B,C;    LD B,D;     LD B,E;    LD B,H;     LD B,L;    LD B,(HL);  LD B,A;
        /* 48 */ LD C,B;     LD C,C;    LD C,D;     LD C,E;    LD C,H;     LD C,L;    LD C,(HL);  LD C,A;
        /* 50 */ LD D,B;     LD D,C;    LD D,D;     LD D,E;    LD D,H;     LD D,L;    LD D,(HL);  LD D,A;
        /* 58 */ LD E,B;     LD E,C;    LD E,D;     LD E,E;    LD E,H;     LD E,L;    LD E,(HL);  LD E,A;
        /* 60 */ LD H,B;     LD H,C;    LD H,D;     LD H,E;    LD H,H;     LD H,L;    LD H,(HL);  LD H,A;
        /* 68 */ LD L,B;     LD L,C;    LD L,D;     LD L,E;    LD L,H;     LD L,L;    LD L,(HL);  LD L,A;
        /* 70 */ LD (HL),B;  LD (HL),C; LD (HL),D;  LD (HL),E; LD (HL),H;  LD (HL),L; HALT;       LD (HL),A;
        /* 78 */ LD A,B;     LD A,C;    LD A,D;     LD A,E;    LD A,H;     LD A,L;    LD A,(HL);  LD A,A;
        /* 80 */ ADD A,B;    ADD A,C;   ADD A,D;    ADD A,E;   ADD A,H;    ADD A,L;   ADD A,(HL); ADD A,A;
        /* 88 */ ADC A,B;    ADC A,C;   ADC A,D;    ADC A,E;   ADC A,H;    ADC A,L;   ADC A,(HL); ADC A,A;
        /* 90 */ SUB B;      SUB C;     SUB D;      SUB E;     SUB H;      SUB L;     SUB (HL);   SUB A;
        /* 98 */ SBC A,B;    SBC A,C;   SBC A,D;    SBC A,E;   SBC A,H;    SBC A,L;   SBC A,(HL); SBC A,A;
        /* A0 */ AND B;      AND C;     AND D;      AND E;     AND H;      AND L;     AND (HL);   AND A;
        /* A8 */ XOR B;      XOR C;     XOR D;      XOR E;     XOR H;      XOR L;     XOR (HL);   XOR A;
        /* B0 */ OR B;       OR C;      OR D;       OR E;      OR H;       OR L;      OR (HL);    OR A;
        /* B8 */ CP B;       CP C;      CP D;       CP E;      CP H;       CP L;      CP (HL);    CP A;
        /* C0 */ RET NZ;     POP BC;    JP NZ,nn;   JP nn;     CALL NZ,nn; PUSH BC;   ADD A,n;    RST 0x00;
        /* C8 */ RET Z;      RET;       JP Z,nn;    CB;        CALL Z,nn;  CALL nn;   ADC A,n;    RST 0x08;
        /* D0 */ RET NC;     POP DE;    JP NC,nn;   UNK;       CALL NC,nn; PUSH DE;   SUB n;      RST 0x10;
        /* D8 */ RET C;      RETI;      JP C,nn;    UNK;       CALL C,nn;  UNK;       SBC A,n;    RST 0x18;
        /* E0 */ LDH (n),A;  POP HL;    LD (C),A;   UNK;       UNK;        PUSH HL;   AND n;      RST 0x20;
        /* E8 */ ADD SP,n;   JP (HL);   LD (nn),A;  UNK;       UNK;        UNK;       XOR n;      RST 0x28;
        /* F0 */ LDH A,(n);  POP AF;    LD A,(C);   DI;        UNK;        PUSH AF;   OR n;       RST 0x30;
        /* F8 */ LD HL,SPn;  LD SP,HL;  LD A,(nn);  EI;        UNK;        UNK;       CP n;       RST 0x38;
    }};
}

#[rustfmt::skip]
macro_rules! instructions_cb {
    ($cont:ident) => { indexing! { $cont @start:
        //       0 / 8       1 / 9      2 / A       3 / B      4 / C       5 / D      6 / E       7 / F
        /* 00 */ RLC B;      RLC C;     RLC D;      RLC E;     RLC H;      RLC L;     RLC (HL);   RLC A;
        /* 08 */ RRC B;      RRC C;     RRC D;      RRC E;     RRC H;      RRC L;     RRC (HL);   RRC A;
        /* 10 */ RL B;       RL C;      RL D;       RL E;      RL H;       RL L;      RL (HL);    RL A;
        /* 18 */ RR B;       RR C;      RR D;       RR E;      RR H;       RR L;      RR (HL);    RR A;
        /* 20 */ SLA B;      SLA C;     SLA D;      SLA E;     SLA H;      SLA L;     SLA (HL);   SLA A;
        /* 28 */ SRA B;      SRA C;     SRA D;      SRA E;     SRA H;      SRA L;     SRA (HL);   SRA A;
        /* 30 */ SWAP B;     SWAP C;    SWAP D;     SWAP E;    SWAP H;     SWAP L;    SWAP (HL);  SWAP A;
        /* 38 */ SRL B;      SRL C;     SRL D;      SRL E;     SRL H;      SRL L;     SRL (HL);   SRL A;
        /* 40 */ BIT 0,B;    BIT 0,C;   BIT 0,D;    BIT 0,E;   BIT 0,H;    BIT 0,L;   BIT 0,(HL); BIT 0,A;
        /* 48 */ BIT 1,B;    BIT 1,C;   BIT 1,D;    BIT 1,E;   BIT 1,H;    BIT 1,L;   BIT 1,(HL); BIT 1,A;
        /* 50 */ BIT 2,B;    BIT 2,C;   BIT 2,D;    BIT 2,E;   BIT 2,H;    BIT 2,L;   BIT 2,(HL); BIT 2,A;
        /* 58 */ BIT 3,B;    BIT 3,C;   BIT 3,D;    BIT 3,E;   BIT 3,H;    BIT 3,L;   BIT 3,(HL); BIT 3,A;
        /* 60 */ BIT 4,B;    BIT 4,C;   BIT 4,D;    BIT 4,E;   BIT 4,H;    BIT 4,L;   BIT 4,(HL); BIT 4,A;
        /* 68 */ BIT 5,B;    BIT 5,C;   BIT 5,D;    BIT 5,E;   BIT 5,H;    BIT 5,L;   BIT 5,(HL); BIT 5,A;
        /* 70 */ BIT 6,B;    BIT 6,C;   BIT 6,D;    BIT 6,E;   BIT 6,H;    BIT 6,L;   BIT 6,(HL); BIT 6,A;
        /* 78 */ BIT 7,B;    BIT 7,C;   BIT 7,D;    BIT 7,E;   BIT 7,H;    BIT 7,L;   BIT 7,(HL); BIT 7,A;
        /* 80 */ RES 0,B;    RES 0,C;   RES 0,D;    RES 0,E;   RES 0,H;    RES 0,L;   RES 0,(HL); RES 0,A;
        /* 88 */ RES 1,B;    RES 1,C;   RES 1,D;    RES 1,E;   RES 1,H;    RES 1,L;   RES 1,(HL); RES 1,A;
        /* 90 */ RES 2,B;    RES 2,C;   RES 2,D;    RES 2,E;   RES 2,H;    RES 2,L;   RES 2,(HL); RES 2,A;
        /* 98 */ RES 3,B;    RES 3,C;   RES 3,D;    RES 3,E;   RES 3,H;    RES 3,L;   RES 3,(HL); RES 3,A;
        /* A0 */ RES 4,B;    RES 4,C;   RES 4,D;    RES 4,E;   RES 4,H;    RES 4,L;   RES 4,(HL); RES 4,A;
        /* A8 */ RES 5,B;    RES 5,C;   RES 5,D;    RES 5,E;   RES 5,H;    RES 5,L;   RES 5,(HL); RES 5,A;
        /* B0 */ RES 6,B;    RES 6,C;   RES 6,D;    RES 6,E;   RES 6,H;    RES 6,L;   RES 6,(HL); RES 6,A;
        /* B8 */ RES 7,B;    RES 7,C;   RES 7,D;    RES 7,E;   RES 7,H;    RES 7,L;   RES 7,(HL); RES 7,A;
        /* C0 */ SET 0,B;    SET 0,C;   SET 0,D;    SET 0,E;   SET 0,H;    SET 0,L;   SET 0,(HL); SET 0,A;
        /* C8 */ SET 1,B;    SET 1,C;   SET 1,D;    SET 1,E;   SET 1,H;    SET 1,L;   SET 1,(HL); SET 1,A;
        /* D0 */ SET 2,B;    SET 2,C;   SET 2,D;    SET 2,E;   SET 2,H;    SET 2,L;   SET 2,(HL); SET 2,A;
        /* D8 */ SET 3,B;    SET 3,C;   SET 3,D;    SET 3,E;   SET 3,H;    SET 3,L;   SET 3,(HL); SET 3,A;
        /* E0 */ SET 4,B;    SET 4,C;   SET 4,D;    SET 4,E;   SET 4,H;    SET 4,L;   SET 4,(HL); SET 4,A;
        /* E8 */ SET 5,B;    SET 5,C;   SET 5,D;    SET 5,E;   SET 5,H;    SET 5,L;   SET 5,(HL); SET 5,A;
        /* F0 */ SET 6,B;    SET 6,C;   SET 6,D;    SET 6,E;   SET 6,H;    SET 6,L;   SET 6,(HL); SET 6,A;
        /* F8 */ SET 7,B;    SET 7,C;   SET 7,D;    SET 7,E;   SET 7,H;    SET 7,L;   SET 7,(HL); SET 7,A;
    }};
}

macro_rules! indexing {
    ($cont:ident @start: $($input:tt)*) => {
        indexing!($cont @indexing: 0 => $($input)* @end_of_input)
    };

    ($cont:ident @indexing: $ix:expr => $mne:ident; $($rest:tt)*) => {
        indexing!($cont @indexing: $ix + 1 => $($rest)* $ix => $mne [];)
    };
    ($cont:ident @indexing: $ix:expr => $mne:ident $opr:tt; $($rest:tt)*) => {
        indexing!($cont @indexing: $ix + 1 => $($rest)* $ix => $mne [$opr];)
    };
    ($cont:ident @indexing: $ix:expr => $mne:ident $dst:tt, $src:tt; $($rest:tt)*) => {
        indexing!($cont @indexing: $ix + 1 => $($rest)* $ix => $mne [$dst, $src];)
    };

    ($cont:ident @indexing: $_:expr => @end_of_input $($ix:expr => $mne:ident $opr:tt; )*) => {
        $cont!($($ix => $mne $opr;)*)
    };
}

impl Cpu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self) -> &mut Register {
        &mut self.reg
    }

    pub fn step(&mut self, ctx: &mut impl Context) {
        self.period += 1;
        while self.cycle < self.period {
            if ctx.check_stall_cpu() {
                self.tick(ctx);
                continue;
            }

            let wake = ctx.check_wake();

            if self.halting {
                // FIXME: halt bug?
                if wake || ctx.interrupt_flag() & ctx.interrupt_enable() != 0 {
                    self.halting = false;
                    debug!("WAKE UP");
                }
                self.tick(ctx);
                self.prev_interrupt_enable = self.interrupt_master_enable;
                continue;
            }

            let pc = self.reg.pc;
            let opc = self.fetch(ctx);
            if self.process_interrupt(ctx, pc) {
                continue;
            }

            if log_enabled!(Level::Trace) {
                self.trace(ctx, pc, opc);
            }
            self.exec_instr(ctx, opc);
        }
    }

    fn process_interrupt(&mut self, ctx: &mut impl Context, ret_addr: u16) -> bool {
        let prev_interrupt_enable = self.prev_interrupt_enable;
        self.prev_interrupt_enable = self.interrupt_master_enable;

        if !prev_interrupt_enable {
            return false;
        }
        if ctx.interrupt_flag() & ctx.interrupt_enable() == 0 {
            return false;
        }

        let prev_if = ctx.interrupt_flag();
        self.interrupt_master_enable = false;
        self.prev_interrupt_enable = false;

        self.push(ctx, (ret_addr >> 8) as u8);
        // Dispatch interrupt vector at this timing
        let addr = self.dispatch_interrupt(ctx);
        self.push(ctx, (ret_addr & 0xff) as u8);

        self.reg.pc = addr;
        debug!(
            "Interrupt occured: IE:{:02X}, IF:{:02X}->{:02X}, ADDR:{:04X}",
            ctx.interrupt_enable(),
            prev_if,
            ctx.interrupt_flag(),
            self.reg.pc
        );

        self.tick(ctx);
        self.tick(ctx);
        self.tick(ctx);
        true
    }

    fn dispatch_interrupt(&mut self, ctx: &mut impl Context) -> u16 {
        let b = ctx.interrupt_flag() & ctx.interrupt_enable();
        if b == 0 {
            // IE (=$FFFF) is written in pushing upper byte of PC, dispatching interrupt vector canceled
            0x0000
        } else {
            let pos = b.trailing_zeros();
            ctx.clear_interrupt_flag_bit(pos as _);
            0x0040 + pos as u16 * 8
        }
    }

    fn exec_instr(&mut self, ctx: &mut impl Context, opc: u8) {
        macro_rules! load {
            (n) => {
                self.fetch(ctx)
            };
            (nn) => {
                self.fetch_u16(ctx)
            };

            (A) => {
                self.reg.a
            };
            (B) => {
                self.reg.b
            };
            (C) => {
                self.reg.c
            };
            (D) => {
                self.reg.d
            };
            (E) => {
                self.reg.e
            };
            (H) => {
                self.reg.h
            };
            (L) => {
                self.reg.l
            };

            (AF) => {
                self.reg.af()
            };
            (BC) => {
                self.reg.bc()
            };
            (DE) => {
                self.reg.de()
            };
            (HL) => {
                self.reg.hl()
            };
            (SP) => {
                self.reg.sp
            };
            (SPn) => {{
                let opr = self.fetch(ctx) as i8 as u16;
                let dst = self.reg.sp;
                let res = dst.wrapping_add(opr);
                self.reg.f.z = false;
                self.reg.f.n = false;
                self.reg.f.h = (opr ^ dst ^ res) & 0x10 != 0;
                self.reg.f.c = (opr ^ dst ^ res) & 0x100 != 0;
                self.tick(ctx);
                res
            }};
            (r8) => {{
                self.fetch(ctx) as i8
            }};

            ((C)) => {
                self.read(ctx, 0xFF00 | self.reg.c as u16)
            };
            ((BC)) => {
                self.read(ctx, self.reg.bc())
            };
            ((DE)) => {
                self.read(ctx, self.reg.de())
            };
            ((HL)) => {{
                let hl = self.reg.hl();
                self.read(ctx, hl)
            }};
            ((^HL)) => {{
                let hl = self.reg.hl();
                self.reg.set_hl(hl.wrapping_add(1));
                self.read(ctx, hl)
            }};
            ((-HL)) => {{
                let hl = self.reg.hl();
                self.reg.set_hl(hl.wrapping_sub(1));
                self.read(ctx, hl)
            }};
            ((nn)) => {{
                let addr = self.fetch_u16(ctx);
                self.read(ctx, addr)
            }};
        }

        macro_rules! store {
            (A, $data:ident) => {{
                self.reg.a = $data;
            }};
            (B, $data:ident) => {{
                self.reg.b = $data;
            }};
            (C, $data:ident) => {{
                self.reg.c = $data;
            }};
            (D, $data:ident) => {{
                self.reg.d = $data;
            }};
            (E, $data:ident) => {{
                self.reg.e = $data;
            }};
            (H, $data:ident) => {{
                self.reg.h = $data;
            }};
            (L, $data:ident) => {{
                self.reg.l = $data;
            }};

            (AF, $data:ident) => {{
                self.reg.set_af($data);
            }};
            (BC, $data:ident) => {
                self.reg.set_bc($data)
            };
            (DE, $data:ident) => {
                self.reg.set_de($data)
            };
            (HL, $data:ident) => {
                self.reg.set_hl($data)
            };
            (SP, $data:ident) => {{
                self.reg.sp = $data;
            }};

            ((C), $data:ident) => {
                self.write(ctx, 0xFF00 | self.reg.c as u16, $data)
            };
            ((BC), $data:ident) => {
                self.write(ctx, self.reg.bc(), $data)
            };
            ((DE), $data:ident) => {
                self.write(ctx, self.reg.de(), $data)
            };
            ((HL), $data:ident) => {{
                let hl = self.reg.hl();
                self.write(ctx, hl, $data);
            }};
            ((^HL), $data:ident) => {{
                let hl = self.reg.hl();
                self.write(ctx, hl, $data);
                self.reg.set_hl(hl.wrapping_add(1));
            }};
            ((-HL), $data:ident) => {{
                let hl = self.reg.hl();
                self.write(ctx, hl, $data);
                self.reg.set_hl(hl.wrapping_sub(1));
            }};
            ((nn), $data:ident) => {{
                let addr = self.fetch_u16(ctx);
                if std::mem::size_of_val(&$data) == 1 {
                    self.write(ctx, addr, $data as u8);
                } else {
                    self.write_u16(ctx, addr, $data as u16);
                }
            }};
        }

        macro_rules! cond {
            (NZ) => {
                !self.reg.f.z
            };
            (Z) => {
                self.reg.f.z
            };
            (NC) => {
                !self.reg.f.c
            };
            (C) => {
                self.reg.f.c
            };
        }

        macro_rules! gen_mne {
            (LD SP, HL) => {{
                self.reg.sp = self.reg.hl();
                self.tick(ctx);
            }};
            (LD $dst:tt, $src:tt) => {{
                let src = load!($src);
                store!($dst, src);
            }};
            (LDH (n), $src:tt) => {{
                let addr = 0xFF00 | self.fetch(ctx) as u16;
                self.write(ctx, addr, load!($src))
            }};
            (LDH $dst:tt, (n)) => {{
                let addr = 0xFF00 | self.fetch(ctx) as u16;
                let data = self.read(ctx, addr);
                store!($dst, data)
            }};

            (PUSH $opr:tt) => {{
                let data = load!($opr);
                self.tick(ctx);
                self.push_u16(ctx, data);
            }};
            (POP $opr:tt) => {{
                let data = self.pop_u16(ctx);
                store!($opr, data);
            }};

            (ADD A, $opr:tt) => {{
                let opr = load!($opr);
                let (res, overflow) = self.reg.a.overflowing_add(opr);
                self.reg.f.n = false;
                self.reg.f.h = (self.reg.a ^ opr ^ res) & 0x10 != 0;
                self.reg.f.c = overflow;
                self.reg.f.z = res == 0;
                self.reg.a = res;
            }};
            (ADD HL, $opr:tt) => {{
                let opr = load!($opr);
                self.tick(ctx);
                let dst = self.reg.hl();
                let (res, overflow) = dst.overflowing_add(opr);
                self.reg.f.n = false;
                self.reg.f.h = (opr ^ dst ^ res) & 0x1000 != 0;
                self.reg.f.c = overflow;
                self.reg.set_hl(res);
            }};
            (ADD SP, $opr:tt) => {{
                let opr = load!($opr) as i8 as u16;
                self.tick(ctx);
                self.tick(ctx);
                let dst = self.reg.sp;
                let res = dst.wrapping_add(opr);
                self.reg.f.z = false;
                self.reg.f.n = false;
                self.reg.f.h = (opr ^ dst ^ res) & 0x10 != 0;
                self.reg.f.c = (opr ^ dst ^ res) & 0x100 != 0;
                self.reg.sp = res;
            }};
            (ADC A, $opr:tt) => {{
                let opr = load!($opr);
                let (res, overflow1) = self.reg.a.overflowing_add(opr);
                let (res, overflow2) = res.overflowing_add(self.reg.f.c as u8);
                self.reg.f.n = false;
                self.reg.f.h = (self.reg.a ^ opr ^ res) & 0x10 != 0;
                self.reg.f.c = overflow1 | overflow2;
                self.reg.f.z = res == 0;
                self.reg.a = res;
            }};
            (SUB $opr:tt) => {{
                let opr = load!($opr);
                let (res, overflow) = self.reg.a.overflowing_sub(opr);
                self.reg.f.n = true;
                self.reg.f.h = (self.reg.a ^ opr ^ res) & 0x10 != 0;
                self.reg.f.c = overflow;
                self.reg.f.z = res == 0;
                self.reg.a = res;
            }};
            (SBC A, $opr:tt) => {{
                let opr = load!($opr);
                let (res, overflow1) = self.reg.a.overflowing_sub(opr);
                let (res, overflow2) = res.overflowing_sub(self.reg.f.c as u8);
                self.reg.f.n = true;
                self.reg.f.h = (self.reg.a ^ opr ^ res) & 0x10 != 0;
                self.reg.f.c = overflow1 | overflow2;
                self.reg.f.z = res == 0;
                self.reg.a = res;
            }};
            (CP $opr:tt) => {{
                let opr = load!($opr);
                let (res, overflow) = self.reg.a.overflowing_sub(opr);
                self.reg.f.n = true;
                self.reg.f.h = (self.reg.a ^ opr ^ res) & 0x10 != 0;
                self.reg.f.c = overflow;
                self.reg.f.z = res == 0;
            }};
            (AND $opr:tt) => {{
                let opr = load!($opr);
                self.reg.a &= opr;
                self.reg.f.z = self.reg.a == 0;
                self.reg.f.n = false;
                self.reg.f.h = true;
                self.reg.f.c = false;
            }};
            (OR $opr:tt) => {{
                let opr = load!($opr);
                self.reg.a |= opr;
                self.reg.f.z = self.reg.a == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = false;
            }};
            (XOR $opr:tt) => {{
                let opr = load!($opr);
                self.reg.a ^= opr;
                self.reg.f.z = self.reg.a == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = false;
            }};

            (INC $opr:tt) => {{
                let opr = load!($opr);
                if std::mem::size_of_val(&opr) == 1 {
                    let res = opr.wrapping_add(1);
                    self.reg.f.z = res == 0;
                    self.reg.f.n = false;
                    self.reg.f.h = (opr ^ res) & 0x10 != 0;
                    store!($opr, res);
                } else {
                    self.tick(ctx);
                    let res = opr.wrapping_add(1);
                    store!($opr, res);
                }
            }};
            (DEC $opr:tt) => {{
                let opr = load!($opr);
                if std::mem::size_of_val(&opr) == 1 {
                    let res = opr.wrapping_sub(1);
                    self.reg.f.z = res == 0;
                    self.reg.f.n = true;
                    self.reg.f.h = (opr ^ res) & 0x10 != 0;
                    store!($opr, res);
                } else {
                    self.tick(ctx);
                    let res = opr.wrapping_sub(1);
                    store!($opr, res);
                }
            }};

            (SWAP $opr:tt) => {{
                let opr = load!($opr);
                let res = opr.rotate_left(4);
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = false;
                store!($opr, res);
            }};
            (DAA) => {{
                let mut adjust = 0;
                adjust |= if self.reg.f.c { 0x60 } else { 0 };
                adjust |= if self.reg.f.h { 0x06 } else { 0 };
                let res = if !self.reg.f.n {
                    adjust |= if self.reg.a & 0x0f > 0x09 { 0x06 } else { 0 };
                    adjust |= if self.reg.a > 0x99 { 0x60 } else { 0 };
                    self.reg.a.wrapping_add(adjust)
                } else {
                    self.reg.a.wrapping_sub(adjust)
                };
                self.reg.a = res;
                self.reg.f.z = res == 0;
                self.reg.f.h = false;
                self.reg.f.c = adjust >= 0x60;
            }};
            (CPL) => {{
                self.reg.a ^= 0xff;
                self.reg.f.n = true;
                self.reg.f.h = true;
            }};
            (CCF) => {{
                self.reg.f.c = !self.reg.f.c;
                self.reg.f.n = false;
                self.reg.f.h = false;
            }};
            (SCF) => {{
                self.reg.f.c = true;
                self.reg.f.n = false;
                self.reg.f.h = false;
            }};
            (NOP) => {{}};
            (HALT) => {{
                self.halting = true;
                debug!("HALT");
            }};
            (STOP) => {{
                self.halting = true;
                ctx.stop();
                debug!("STOP");
            }};
            (DI) => {{
                self.prev_interrupt_enable = false;
                self.interrupt_master_enable = false;
            }};
            (EI) => {{
                self.interrupt_master_enable = true;
            }};

            (RLCA) => {
                // RLCA always reset Z flag
                // RLC A sets Z flag if result is zero
                gen_mne!(RLC A, false)
            };
            (RLA) => {
                gen_mne!(RL A, false)
            };
            (RRCA) => {
                gen_mne!(RRC A, false)
            };
            (RRA) => {
                gen_mne!(RR A, false)
            };
            (RLC $opr:tt $(, $f:literal)?) => {{
                let opr = load!($opr);
                let res = opr.rotate_left(1);
                self.reg.f.z = res == 0 $(&& $f)*;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (opr & 0x80) != 0;
                store!($opr, res);
            }};
            (RL $opr:tt $(, $f:literal)?) => {{
                let opr = load!($opr);
                let res = opr << 1 | self.reg.f.c as u8;
                self.reg.f.z = res == 0 $(&& $f)*;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (opr & 0x80) != 0;
                store!($opr, res);
            }};
            (RRC $opr:tt $(, $f:literal)?) => {{
                let opr = load!($opr);
                let res = opr.rotate_right(1);
                self.reg.f.z = res == 0 $(&& $f)*;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (opr & 0x01) != 0;
                store!($opr, res);
            }};
            (RR $opr:tt $(, $f:literal)?) => {{
                let opr = load!($opr);
                let res = opr >> 1 | (self.reg.f.c as u8) << 7;
                self.reg.f.z = res == 0 $(&& $f)*;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (opr & 0x01) != 0;
                store!($opr, res);
            }};
            (SLA $opr:tt) => {{
                let opr = load!($opr);
                let res = opr << 1;
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (opr & 0x80) != 0;
                store!($opr, res);
            }};
            (SRA $opr:tt) => {{
                let opr = load!($opr);
                let res = opr >> 1 | (opr & 0x80);
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (opr & 0x01) != 0;
                store!($opr, res);
            }};
            (SRL $opr:tt) => {{
                let opr = load!($opr);
                let res = opr >> 1;
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (opr & 0x01) != 0;
                store!($opr, res);
            }};
            (BIT $bit:literal, $opr:tt) => {{
                let opr = load!($opr);
                self.reg.f.z = (opr & (1 << $bit)) == 0;
                self.reg.f.n = false;
                self.reg.f.h = true;
            }};
            (SET $bit:literal, $opr:tt) => {{
                let opr = load!($opr);
                let res = opr | (1 << $bit);
                store!($opr, res);
            }};
            (RES $bit:literal, $opr:tt) => {{
                let opr = load!($opr);
                let res = opr & !(1 << $bit);
                store!($opr, res);
            }};

            (JP nn) => {{
                self.reg.pc = load!(nn);
                self.tick(ctx);
            }};
            (JP (HL)) => {{
                self.reg.pc = self.reg.hl();
            }};
            (JP $cc:tt, nn) => {{
                let addr = load!(nn);
                if cond!($cc) {
                    self.reg.pc = addr;
                    self.tick(ctx);
                }
            }};
            (JR $opr:tt) => {{
                let r = load!($opr) as u16;
                self.reg.pc = self.reg.pc.wrapping_add(r);
                self.tick(ctx);
            }};
            (JR $cc:tt, $opr:tt) => {{
                let r = load!($opr) as u16;
                if cond!($cc) {
                    self.reg.pc = self.reg.pc.wrapping_add(r);
                    self.tick(ctx);
                }
            }};
            (CALL $opr:tt) => {{
                let addr = load!($opr);
                self.tick(ctx);
                self.push_u16(ctx, self.reg.pc);
                self.reg.pc = addr;
            }};
            (CALL $cc:tt, $opr:tt) => {{
                let addr = load!($opr);
                if cond!($cc) {
                    self.tick(ctx);
                    self.push_u16(ctx, self.reg.pc);
                    self.reg.pc = addr;
                }
            }};
            (RST $opr:expr) => {{
                self.tick(ctx);
                self.push_u16(ctx, self.reg.pc);
                self.reg.pc = $opr;
            }};

            (RET) => {{
                self.reg.pc = self.pop_u16(ctx);
                self.tick(ctx);
            }};
            (RET $cc:tt) => {{
                self.tick(ctx);
                if cond!($cc) {
                    self.reg.pc = self.pop_u16(ctx);
                    self.tick(ctx);
                }
            }};
            (RETI) => {{
                self.reg.pc = self.pop_u16(ctx);
                self.tick(ctx);
                self.interrupt_master_enable = true;
            }};

            (UNK) => {
                panic!("Unknown instruction")
            };

            (CB) => {
                instructions_cb!(gen_code_cb)
            };
        }

        macro_rules! gen_instr {
            ($mne:ident []) => {
                gen_mne!($mne)
            };
            ($mne:ident [$opr:tt]) => {{
                gen_mne!($mne $opr)
            }};
            ($mne:ident [$dst:tt, $src:tt]) => {{
                gen_mne!($mne $dst, $src)
            }};
        }

        macro_rules! gen_code {
            ($($ix:expr => $mne:ident $opr:tt;)*) => {
                match opc {
                    $( ConstEval::<{$ix}>::VALUE => gen_instr!($mne $opr), )*
                }
            };
        }

        macro_rules! gen_code_cb {
            ($($ix:expr => $mne:ident $opr:tt;)*) => {{
                let opc_cb = self.fetch(ctx);
                match opc_cb {
                    $( ConstEval::<{$ix}>::VALUE => gen_instr!($mne $opr), )*
                }
            }};
        }

        instructions!(gen_code);
    }
}

impl Cpu {
    fn tick(&mut self, ctx: &mut impl Context) {
        self.cycle += 1;
        ctx.tick();
    }

    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        let data = ctx.read(addr);
        self.tick(ctx);
        data
    }

    fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        ctx.write(addr, data);
        self.tick(ctx);
    }

    fn write_u16(&mut self, ctx: &mut impl Context, addr: u16, data: u16) {
        self.write(ctx, addr, (data & 0xFF) as u8);
        self.write(ctx, addr.wrapping_add(1), (data >> 8) as u8);
    }

    fn fetch(&mut self, ctx: &mut impl Context) -> u8 {
        let ret = self.read(ctx, self.reg.pc);
        self.reg.pc += 1;
        ret
    }

    fn fetch_u16(&mut self, ctx: &mut impl Context) -> u16 {
        let lo = self.fetch(ctx);
        let hi = self.fetch(ctx);
        lo as u16 | (hi as u16) << 8
    }

    fn push(&mut self, ctx: &mut impl Context, data: u8) {
        self.reg.sp -= 1;
        self.write(ctx, self.reg.sp, data);
    }

    fn push_u16(&mut self, ctx: &mut impl Context, data: u16) {
        self.push(ctx, (data >> 8) as u8);
        self.push(ctx, (data & 0xFF) as u8);
    }

    fn pop(&mut self, ctx: &mut impl Context) -> u8 {
        let ret = self.read(ctx, self.reg.sp);
        self.reg.sp += 1;
        ret
    }

    fn pop_u16(&mut self, ctx: &mut impl Context) -> u16 {
        let lo = self.pop(ctx);
        let hi = self.pop(ctx);
        lo as u16 | (hi as u16) << 8
    }
}

impl Cpu {
    fn trace(&mut self, ctx: &mut impl Context, pc: u16, opc: u8) {
        let opr1 = ctx.read_immutable(pc.wrapping_add(1));
        let opr2 = ctx.read_immutable(pc.wrapping_add(2));

        let (asm, op_len) = disasm(pc, opc, opr1, opr2);

        let tos = |mb: Option<u8>| mb.map_or("??".to_string(), |x| format!("{x:02X}"));
        let bytes = match op_len {
            1 => format!("{:02X}", opc),
            2 => format!("{:02X} {}", opc, tos(opr1)),
            3 => format!("{:02X} {} {}", opc, tos(opr1), tos(opr2)),
            _ => unreachable!(),
        };

        use crate::consts::*;

        trace!(
            "{pc:04X}: {bytes:8} | {asm:20} | \
            A:{a:02X} B:{b:02X} C:{c:02X} D:{d:02X} E:{e:02X} H:{h:02X} L:{l:02X} \
            SP:{sp:04X} F:{zf}{nf}{hf}{cf} IME:{ime} IE:{ie:02X} IF:{inf:02X} CYC:{frm}:{ly:03}:{lx:03}",
            a = self.reg.a,
            b = self.reg.b,
            c = self.reg.c,
            d = self.reg.d,
            e = self.reg.e,
            h = self.reg.h,
            l = self.reg.l,
            sp = self.reg.sp,
            zf = if self.reg.f.z { 'Z' } else { '.' },
            nf = if self.reg.f.n { 'N' } else { '.' },
            hf = if self.reg.f.h { 'H' } else { '.' },
            cf = if self.reg.f.c { 'C' } else { '.' },
            ime = self.interrupt_master_enable as u8,
            ie = ctx.interrupt_enable(),
            inf = ctx.interrupt_flag(),
            frm = self.cycle / CPU_CLOCK_PER_LINE / LINES_PER_FRAME,
            ly = self.cycle / CPU_CLOCK_PER_LINE % LINES_PER_FRAME,
            lx = self.cycle % CPU_CLOCK_PER_LINE,
        );
    }
}

#[rustfmt::skip]
const HWREG_NAME: &[(u16, &str)] = &[
    (0xFF00, "P1"), (0xFF01, "SB"), (0xFF02, "SC"),
    (0xFF04, "DIV"), (0xFF05, "TIMA"), (0xFF06, "TMA"), (0xFF07, "TAC"), (0xFF0F, "IF"),

    (0xFF10, "NR10"), (0xFF11, "NR11"), (0xFF12, "NR12"), (0xFF13, "NR13"), (0xFF14, "NR14"),
    (0xFF16, "NR21"), (0xFF17, "NR22"), (0xFF18, "NR23"), (0xFF19, "NR24"),
    (0xFF1A, "NR30"), (0xFF1B, "NR31"), (0xFF1C, "NR32"), (0xFF1D, "NR33"), (0xFF1E, "NR34"),
    (0xFF20, "NR41"), (0xFF21, "NR42"), (0xFF22, "NR43"), (0xFF23, "NR44"),
    (0xFF24, "NR50"), (0xFF25, "NR51"), (0xFF26, "NR52"),

    (0xFF40, "LCDC"), (0xFF41, "STAT"), (0xFF42, "SCY"), (0xFF43, "SCX"),
    (0xFF44, "LY"), (0xFF45, "LYC"), (0xFF46, "DMA"), (0xFF47, "BGP"),
    (0xFF48, "OBP0"), (0xFF49, "OBP1"), (0xFF4A, "WY"), (0xFF4B, "WX"), 

    (0xFF4D, "KEY1"), (0xFF4F, "VBK"), (0xFF50, "BOOT"),
    (0xFF51, "HDMA1"), (0xFF52, "HDMA2"), (0xFF53, "HDMA3"), (0xFF54, "HDMA4"), (0xFF55, "HDMA5"),
    (0xFF56, "RP"),
    
    (0xFF68, "BCPS"), (0xFF69, "BCPD"), (0xFF6A, "OCPS"), (0xFF6B, "OCPD"),
    (0xFF70, "SVBK"), (0xFF76, "PCM12"), (0xFF77, "PCM34"),
    
    (0xFFFF, "IE"),
];

fn hwreg_name(addr: u8) -> Option<&'static str> {
    HWREG_NAME
        .iter()
        .find(|r| addr as u16 | 0xFF00 == r.0)
        .map(|r| r.1)
}

fn disasm(pc: u16, opc: u8, opr1: Option<u8>, opr2: Option<u8>) -> (String, usize) {
    let opc = opc;
    let opr1 = opr1;
    let opr2 = opr2;
    let mut bytes = 1;

    macro_rules! gen_opr {
        ((^HL)) => {
            "(HL+)"
        };
        ((-HL)) => {
            "(HL-)"
        };

        (SPn) => {{
            bytes += 1;
            opr1.map_or_else(|| "SP+??".to_string(), |opr| format!("SP{:+}", opr as i8))
        }};

        (n) => {{
            bytes += 1;
            opr1.map_or_else(|| "$??".to_string(), |opr| format!("${opr:02X}"))
        }};
        ((n)) => {{
            bytes += 1;
            opr1.map_or_else(
                || "($??)".to_string(),
                |opr| {
                    hwreg_name(opr).map_or_else(
                        || format!("(${opr:02X})"),
                        |name| format!("(<{name}=${opr:02X})"),
                    )
                },
            )
        }};
        (r8) => {{
            bytes += 1;
            opr1.map_or_else(
                || "$????".to_string(),
                |opr| format!("${:04X}", pc.wrapping_add(2).wrapping_add(opr as i8 as u16)),
            )
        }};
        (nn) => {{
            bytes += 2;
            opr1.and_then(|opr1| opr2.map(|opr2| format!("${:02X}{:02X}", opr2, opr1)))
                .unwrap_or_else(|| "$????".to_string())
        }};
        ((nn)) => {{
            bytes += 2;
            opr1.and_then(|opr1| opr2.map(|opr2| format!("(${:02X}{:02X})", opr2, opr1)))
                .unwrap_or_else(|| "($????)".to_string())
        }};

        ($n:literal) => {
            format!("{:02X}H", $n)
        };

        ($opr:ident) => {
            stringify!($opr)
        };
        (($opr:ident)) => {
            stringify!(($opr))
        };
    }

    macro_rules! gen_disasm {
        ($($ix:expr => $mne:ident $opr:tt;)*) => {
            match opc {
                $( ConstEval::<{$ix}>::VALUE => {
                    let asm = gen_disasm!(@generate: $mne $opr);
                    (asm, bytes)
                })*
            }
        };

        (@generate: CB []) => {
            instructions_cb!(gen_disasm_cb)
        };

        (@generate: $mne:ident []) => {
            stringify!($mne).to_string()
        };
        (@generate: $mne:ident [$opr:tt]) => {
            format!("{} {}", stringify!($mne), gen_opr!($opr))
        };
        (@generate: $mne:ident [$dst:tt, $src:tt]) => {
            format!("{} {}, {}", stringify!($mne), gen_opr!($dst), gen_opr!($src))
        };
    }

    macro_rules! gen_disasm_cb {
        ($($ix:expr => $mne:ident $opr:tt;)*) => {{
            bytes += 1;
            match opr1 {
                $( Some(ConstEval::<{$ix}>::VALUE) => {
                    gen_disasm_cb!(@generate: $mne $opr)
                })*
                None => format!("???"),
            }
        }};

        (@generate: $mne:ident [$opr:tt]) => {
            format!("{} {}", stringify!($mne), gen_opr!($opr))
        };
        (@generate: $mne:ident [$n:literal, $opr:tt]) => {
            format!("{} {}, {}", stringify!($mne), $n, gen_opr!($opr))
        };
    }

    instructions!(gen_disasm)
}
