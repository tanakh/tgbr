use crate::{bus::Bus, util::Ref};
use log::{log_enabled, trace, Level};

use bitvec::prelude::*;

pub struct Cpu {
    halting: bool,
    interrupt_enable: bool,
    prev_interrupt_enable: bool,
    pub reg: Register,
    counter: u64,
    world: u64,
    bus: Ref<Bus>,
}

#[derive(Default)]
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

#[derive(Default)]
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
    ($cont:ident) => { $cont! { @start:
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

trait ConstEval<const T: u8> {
    const VALUE: u8 = T;
}

impl Cpu {
    pub fn new(bus: &Ref<Bus>) -> Self {
        Self {
            reg: Register::default(),
            halting: false,
            interrupt_enable: false,
            prev_interrupt_enable: false,
            counter: 0,
            world: 0,
            bus: Ref::clone(bus),
        }
    }

    pub fn tick(&mut self) {
        self.world += 1;
        while self.counter < self.world {
            if self.prev_interrupt_enable {
                // TODO: check interrupt
            }
            self.prev_interrupt_enable = self.interrupt_enable;

            self.exec_instr();
        }
    }

    fn exec_instr(&mut self) {
        let opc = 0_u8;

        if log_enabled!(Level::Trace) {
            self.trace(opc);
        }

        macro_rules! gen_code {
            (@start: $($input:tt)*) => {
                gen_code!(@indexing: 0 => $($input)* @end_of_input)
            };

            (@indexing: $ix:expr => $mne:ident; $($rest:tt)*) => {
                gen_code!(@indexing: $ix + 1 => $($rest)* $ix => $mne [];)
            };
            (@indexing: $ix:expr => $mne:ident $opr:tt; $($rest:tt)*) => {
                gen_code!(@indexing: $ix + 1 => $($rest)* $ix => $mne [$opr];)
            };
            (@indexing: $ix:expr => $mne:ident $dst:tt, $src:tt; $($rest:tt)*) => {
                gen_code!(@indexing: $ix + 1 => $($rest)* $ix => $mne [$dst, $src];)
            };

            (@indexing: $_:expr => @end_of_input $($ix:expr => $mne:ident $opr:tt; )*) => {{
                struct ConstEval<const V: u8>;

                impl<const V: u8> ConstEval<V> {
                    const VALUE: u8 = V;
                }

                match opc {
                    $( ConstEval::<{$ix}>::VALUE => gen_instr!($mne $opr), )*
                }
            }};
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

        macro_rules! load {
            (n) => {
                self.fetch()
            };
            (nn) => {
                self.fetch_u16()
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
                // FIXME: set flags
                self.reg.sp.wrapping_add(self.fetch() as i8 as u16)
            }};
            (r8) => {{
                self.fetch() as i8
            }};

            ((C)) => {
                self.read(0xFF00 | self.reg.c as u16)
            };
            ((BC)) => {
                self.read(self.reg.bc())
            };
            ((DE)) => {
                self.read(self.reg.de())
            };
            ((HL)) => {{
                let hl = self.reg.hl();
                self.read(hl)
            }};
            ((^HL)) => {{
                let hl = self.reg.hl();
                self.reg.set_hl(hl.wrapping_add(1));
                self.read(hl)
            }};
            ((-HL)) => {{
                let hl = self.reg.hl();
                self.reg.set_hl(hl.wrapping_sub(1));
                self.read(hl)
            }};
            ((nn)) => {{
                let addr = self.fetch_u16();
                self.read(addr)
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
                self.reg.h = $data;
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
                self.write(0xFF00 | self.reg.c as u16, $data)
            };
            ((BC), $data:ident) => {
                self.write(self.reg.bc(), $data)
            };
            ((DE), $data:ident) => {
                self.write(self.reg.de(), $data)
            };
            ((HL), $data:ident) => {{
                let hl = self.reg.hl();
                self.write(hl, $data);
            }};
            ((^HL), $data:ident) => {{
                let hl = self.reg.hl();
                self.write(hl, $data);
                self.reg.set_hl(hl.wrapping_add(1));
            }};
            ((-HL), $data:ident) => {{
                let hl = self.reg.hl();
                self.write(hl, $data);
                self.reg.set_hl(hl.wrapping_sub(1));
            }};
            ((nn), $data:ident) => {{
                let addr = self.fetch_u16();
                if std::mem::size_of_val(&$data) == 8 {
                    self.write(addr, $data as u8);
                } else {
                    self.write_u16(addr, $data as u16);
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
            (LD $dst:tt, $src:tt) => {{
                let src = load!($src);
                store!($dst, src);
            }};
            (LDH (n), $src:tt) => {{
                let addr = 0xFF00 | self.fetch() as u16;
                self.write(addr, load!($src))
            }};
            (LDH $dst:tt, (n)) => {{
                let addr = 0xFF00 | self.fetch() as u16;
                let data = self.read(addr);
                store!($dst, data)
            }};

            (PUSH $opr:tt) => {{
                let data = load!($opr);
                self.push_u16(data);
            }};
            (POP $opr:tt) => {{
                let data = self.pop_u16();
                store!($opr, data);
            }};

            (ADD A, $opr:tt) => {{
                let opr = load!($opr);
                let res = self.reg.a as u16 + opr as u16;
                self.reg.f.n = false;
                self.reg.f.h = (self.reg.a ^ opr ^ res as u8) & 0x10 != 0;
                self.reg.f.c = res > 0xff;
                self.reg.f.z = res & 0xff == 0;
                self.reg.a = res as u8;
            }};
            (ADD HL, $opr:tt) => {{
                let opr = load!($opr);
                let dst = self.reg.hl();
                let res = dst as u32 + opr as u32;
                self.reg.f.n = false;
                self.reg.f.h = (opr ^ dst ^ res as u16) & 0x1000 != 0;
                self.reg.f.c = res > 0xffff;
                self.reg.set_hl(res as u16);
            }};
            (ADD SP, $opr:tt) => {{
                let opr = load!($opr) as i8 as u16;
                let dst = self.reg.hl();
                let res = dst.wrapping_add(opr);
                self.reg.f.z = false;
                self.reg.f.n = false;
                self.reg.f.h = (opr ^ dst ^ res) & 0x10 != 0;
                self.reg.f.c = (opr ^ dst ^ res) & 0x100 != 0;
                self.reg.set_hl(res as u16);
            }};
            (ADC A, $opr:tt) => {{
                let opr = load!($opr);
                let res = self.reg.a as u16 + opr as u16 + self.reg.f.c as u16;
                self.reg.f.n = false;
                self.reg.f.h = (self.reg.a ^ opr ^ res as u8) & 0x10 != 0;
                self.reg.f.c = res > 0xff;
                self.reg.f.z = res & 0xff == 0;
                self.reg.a = res as u8;
            }};
            (SUB $opr:tt) => {{
                let opr = load!($opr);
                let res = (self.reg.a as u16).wrapping_sub(opr as u16);
                self.reg.f.n = true;
                self.reg.f.h = (self.reg.a ^ opr ^ res as u8) & 0x10 == 0;
                self.reg.f.c = res <= 0xff;
                self.reg.f.z = res & 0xff == 0;
                self.reg.a = res as u8;
            }};
            (SBC A, $opr:tt) => {{
                let opr = load!($opr);
                let res = (self.reg.a as u16)
                    .wrapping_sub(opr as u16)
                    .wrapping_add(self.reg.f.c as u16);
                self.reg.f.n = true;
                self.reg.f.h = (self.reg.a ^ opr ^ res as u8) & 0x10 == 0;
                self.reg.f.c = res <= 0xff;
                self.reg.f.z = res & 0xff == 0;
                self.reg.a = res as u8;
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
            (CP $opr:tt) => {{
                let opr = load!($opr);
                let res = (self.reg.a as u16).wrapping_sub(opr as u16);
                self.reg.f.n = true;
                self.reg.f.h = (self.reg.a ^ opr ^ res as u8) & 0x10 == 0;
                self.reg.f.c = res <= 0xff;
                self.reg.f.z = res & 0xff == 0;
            }};

            (INC $opr:tt) => {{
                let opr = load!($opr);
                if std::mem::size_of_val(&opr) == 8 {
                    let res = opr.wrapping_add(1);
                    self.reg.f.z = res == 0;
                    self.reg.f.n = false;
                    self.reg.f.h = (opr ^ res) & 0x10 != 0;
                    store!($opr, res);
                } else {
                    let res = opr.wrapping_add(1);
                    store!($opr, res);
                }
            }};
            (DEC $opr:tt) => {{
                let opr = load!($opr);
                if std::mem::size_of_val(&opr) == 8 {
                    let res = opr.wrapping_sub(1);
                    self.reg.f.z = res == 0;
                    self.reg.f.n = false;
                    self.reg.f.h = (opr ^ res) & 0x10 == 0;
                    store!($opr, res);
                } else {
                    let res = opr.wrapping_sub(1);
                    store!($opr, res);
                }
            }};

            // SWAP n
            (DAA) => {{
                todo!("DAA")
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
            }};
            (STOP) => {
                todo!("STOP")
            };
            (DI) => {{
                self.interrupt_enable = false;
            }};
            (EI) => {{
                self.interrupt_enable = true;
            }};

            (RLCA) => {{
                let res = self.reg.a.rotate_left(1);
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (self.reg.a & 0x80) != 0;
                self.reg.a = res;
            }};
            (RLA) => {{
                let res = self.reg.a << 1 | self.reg.f.c as u8;
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (self.reg.a & 0x80) != 0;
                self.reg.a = res;
            }};
            (RRCA) => {{
                let res = self.reg.a.rotate_right(1);
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (self.reg.a & 0x01) != 0;
                self.reg.a = res;
            }};
            (RRA) => {{
                let res = self.reg.a >> 1 | (self.reg.f.c as u8) << 7;
                self.reg.f.z = res == 0;
                self.reg.f.n = false;
                self.reg.f.h = false;
                self.reg.f.c = (self.reg.a & 0x01) != 0;
                self.reg.a = res;
            }};
            // RLC n
            // RL n
            // RRC n
            // RR n
            // SLA n
            // SRA n
            // SRL n
            // BIT b, r
            // SET b, r
            // RES b, r
            //
            (JP nn) => {{
                self.reg.pc = load!(nn);
            }};
            (JP (HL)) => {{
                self.reg.pc = self.reg.hl();
            }};
            (JP $cc:tt, nn) => {{
                let addr = load!(nn);
                if cond!($cc) {
                    self.reg.pc = addr;
                }
            }};
            (JR $opr:tt) => {{
                let r = load!($opr) as u16;
                self.reg.pc = self.reg.pc.wrapping_add(r);
            }};
            (JR $cc:tt, $opr:tt) => {{
                let r = load!($opr) as u16;
                if cond!($cc) {
                    self.reg.pc = self.reg.pc.wrapping_add(r);
                }
            }};
            (CALL $opr:tt) => {{
                let addr = load!($opr);
                self.push_u16(self.reg.pc);
                self.reg.pc = addr;
            }};
            (CALL $cc:tt, $opr:tt) => {{
                let addr = load!($opr);
                if cond!($cc) {
                    self.push_u16(self.reg.pc);
                    self.reg.pc = addr;
                }
            }};
            (RST $opr:expr) => {{
                self.push_u16(self.reg.pc);
                self.reg.pc = $opr;
            }};

            (RET) => {{
                self.reg.pc = self.pop_u16();
            }};
            (RET $cc:tt) => {{
                if cond!($cc) {
                    self.reg.pc = self.pop_u16();
                }
            }};
            (RETI) => {{
                self.reg.pc = self.pop_u16();
                self.interrupt_enable = true;
            }};

            (UNK) => {
                todo!("Unknown instruction")
            };

            (CB) => {
                todo!("CB prefixed")
            };
        }

        instructions!(gen_code);
    }
}

impl Cpu {
    fn read(&mut self, addr: u16) -> u8 {
        self.counter += 1;
        self.bus.borrow_mut().read(addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.counter += 1;
        self.bus.borrow_mut().write(addr, data)
    }

    fn write_u16(&mut self, addr: u16, data: u16) {
        self.write(addr, (data & 0xFF) as u8);
        self.write(addr.wrapping_add(1), (data >> 8) as u8);
    }

    fn fetch(&mut self) -> u8 {
        let ret = self.read(self.reg.pc);
        self.reg.pc += 1;
        ret
    }

    fn fetch_u16(&mut self) -> u16 {
        let lo = self.read(self.reg.pc);
        let hi = self.read(self.reg.pc.wrapping_add(1));
        self.reg.pc += 2;
        lo as u16 | (hi as u16) << 8
    }

    fn push(&mut self, data: u8) {
        self.reg.sp -= 1;
        self.write(self.reg.sp, data);
    }

    fn push_u16(&mut self, data: u16) {
        self.push((data >> 8) as u8);
        self.push((data & 0xFF) as u8);
    }

    fn pop(&mut self) -> u8 {
        let ret = self.read(self.reg.sp);
        self.reg.sp += 1;
        ret
    }

    fn pop_u16(&mut self) -> u16 {
        let lo = self.pop();
        let hi = self.pop();
        lo as u16 | (hi as u16) << 8
    }
}

impl Cpu {
    fn trace(&mut self, opc: u8) {
        let pc = self.reg.pc;
        let opr1 = self.bus.borrow_mut().read_immutable(pc.wrapping_add(1));
        let opr2 = self.bus.borrow_mut().read_immutable(pc.wrapping_add(2));

        let (asm, op_len) = disasm(pc, opc, opr1, opr2);

        let tos = |mb: Option<u8>| mb.map_or("??".to_string(), |x| format!("{x:02X}"));
        let bytes = match op_len {
            1 => format!("{:02X}", opc),
            2 => format!("{:02X} {}", opc, tos(opr1)),
            3 => format!("{:02X} {} {}", opc, tos(opr1), tos(opr2)),
            _ => unreachable!(),
        };

        trace!(
            "{pc:04X}  {bytes:8} {asm:16} | \
            A:{a:02X} B:{b:02X} C:{c:02X} D:{d:02X} E:{e:02X} H:{h:02X} L:{l:02X} \
            SP:{sp:04X} F:{zf}{nf}{hf}{cf}",
            a = self.reg.a,
            b = self.reg.b,
            c = self.reg.c,
            d = self.reg.d,
            e = self.reg.e,
            h = self.reg.h,
            l = self.reg.l,
            sp = self.reg.sp,
            zf = self.reg.f.z,
            nf = self.reg.f.n,
            hf = self.reg.f.h,
            cf = self.reg.f.c,
        );
    }
}

fn disasm(pc: u16, opc: u8, opr1: Option<u8>, opr2: Option<u8>) -> (String, usize) {
    let opc = opc;
    let opr1 = opr1;
    let opr2 = opr2;
    let mut bytes = 1;

    macro_rules! gen_disasm {
        (@start: $($input:tt)*) => {
            gen_disasm!(@indexing: 0 => $($input)* @end_of_input)
        };

        (@indexing: $ix:expr => $mne:ident; $($rest:tt)*) => {
            gen_disasm!(@indexing: $ix + 1 => $($rest)* $ix => $mne [];)
        };
        (@indexing: $ix:expr => $mne:ident $opr:tt; $($rest:tt)*) => {
            gen_disasm!(@indexing: $ix + 1 => $($rest)* $ix => $mne [$opr];)
        };
        (@indexing: $ix:expr => $mne:ident $dst:tt, $src:tt; $($rest:tt)*) => {
            gen_disasm!(@indexing: $ix + 1 => $($rest)* $ix => $mne [$dst, $src];)
        };


        (@indexing: $_:expr => @end_of_input $($ix:expr => $mne:ident $opr:tt;)*) => {{
            struct ConstEval<const V: u8>;

            impl<const V: u8> ConstEval<V> {
                const VALUE: u8 = V;
            }

            match opc {
                $( ConstEval::<{$ix}>::VALUE => {
                    let asm = gen_disasm!(@generate: $mne $opr);
                    (asm, bytes)
                })*
            }
        }};

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

    macro_rules! gen_opr {
        ((^HL)) => {
            "(HL+)"
        };
        ((-HL)) => {
            "(HL-)"
        };

        (SPn) => {{
            bytes += 1;
            opr1.map_or("SP+??".to_string(), |opr| format!("SP{:+}", opr as i8))
        }};

        (n) => {{
            bytes += 1;
            opr1.map_or("$??".to_string(), |opr| format!("${:02X}", opr as i8))
        }};
        ((n)) => {{
            bytes += 1;
            opr1.map_or("($??)".to_string(), |opr| format!("(${:02X})", opr as i8))
        }};
        (r8) => {{
            bytes += 1;
            opr1.map_or("$????".to_string(), |opr| {
                format!("${:04X}", pc.wrapping_add(2).wrapping_add(opr as i8 as u16))
            })
        }};
        (nn) => {{
            bytes += 2;
            opr1.and_then(|opr1| opr2.map(|opr2| format!("${:02X}{:02X}", opr1, opr2)))
                .unwrap_or("$????".to_string())
        }};
        ((nn)) => {{
            bytes += 2;
            opr1.and_then(|opr1| opr2.map(|opr2| format!("(${:02X}{:02X})", opr1, opr2)))
                .unwrap_or("($????)".to_string())
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

    instructions!(gen_disasm)
}
