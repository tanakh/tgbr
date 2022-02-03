use crate::{
    bus::Bus,
    util::{ConstEval, Ref},
};
use log::{info, log_enabled, trace, Level};

use bitvec::prelude::*;

pub struct Cpu {
    halting: bool,
    interrupt_master_enable: bool,
    prev_interrupt_enable: bool,
    reg: Register,
    counter: u64,
    world: u64,
    interrupt_enable: Ref<u8>,
    interrupt_flag: Ref<u8>,
    prev_interrupt_flag: u8,
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
    pub fn new(bus: &Ref<Bus>, interrupt_enable: &Ref<u8>, interrupt_flag: &Ref<u8>) -> Self {
        Self {
            reg: Register::default(),
            halting: false,
            interrupt_master_enable: false,
            prev_interrupt_enable: false,
            counter: 0,
            world: 0,
            interrupt_enable: Ref::clone(interrupt_enable),
            interrupt_flag: Ref::clone(interrupt_flag),
            prev_interrupt_flag: 0,
            bus: Ref::clone(bus),
        }
    }

    pub fn register(&mut self) -> &mut Register {
        &mut self.reg
    }

    pub fn tick(&mut self) {
        self.world += 1;
        while self.counter < self.world {
            if self.process_interrupt() {
                continue;
            }
            if self.halting {
                let b = *self.interrupt_flag.borrow() & *self.interrupt_enable.borrow();
                if b == 0 {
                    self.counter += 1;
                    continue;
                }
                self.halting = false;
                info!("WAKE UP");
                // FIXME: halt bug?
            }
            self.prev_interrupt_enable = self.interrupt_master_enable;
            self.exec_instr();
        }
    }

    fn process_interrupt(&mut self) -> bool {
        let interrupt_flag = *self.interrupt_flag.borrow();
        let interrupt_flag_rise = !self.prev_interrupt_flag & interrupt_flag;
        self.prev_interrupt_flag = interrupt_flag;

        if !self.prev_interrupt_enable {
            return false;
        }
        // let b = interrupt_flag_rise & *self.interrupt_enable.borrow();
        let b = interrupt_flag & *self.interrupt_enable.borrow();
        if b == 0 {
            return false;
        }
        for i in 0..5 {
            if b & (1 << i) != 0 {
                info!("INT {:02X} occured", 0x40 + i * 8);
                self.interrupt_master_enable = false;
                *self.interrupt_flag.borrow_mut() &= !(1 << i);
                self.push_u16(self.reg.pc);
                self.reg.pc = 0x40 + i * 8;
                self.counter += 1;
                self.halting = false;
                return true;
            }
        }
        unreachable!()
    }

    fn exec_instr(&mut self) {
        let pc = self.reg.pc;
        let opc = self.fetch();

        if log_enabled!(Level::Trace) {
            self.trace(pc, opc);
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
                let opr = self.fetch() as i8 as u16;
                let dst = self.reg.sp;
                let res = dst.wrapping_add(opr);
                self.reg.f.z = false;
                self.reg.f.n = false;
                self.reg.f.h = (opr ^ dst ^ res) & 0x10 != 0;
                self.reg.f.c = (opr ^ dst ^ res) & 0x100 != 0;
                self.counter += 1;
                res
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
                if std::mem::size_of_val(&$data) == 1 {
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
            (LD SP, HL) => {{
                self.reg.sp = self.reg.hl();
                self.counter += 1;
            }};
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
                self.counter += 1;
            }};
            (POP $opr:tt) => {{
                let data = self.pop_u16();
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
                self.counter += 1;
                let dst = self.reg.hl();
                let (res, overflow) = dst.overflowing_add(opr);
                self.reg.f.n = false;
                self.reg.f.h = (opr ^ dst ^ res) & 0x1000 != 0;
                self.reg.f.c = overflow;
                self.reg.set_hl(res);
            }};
            (ADD SP, $opr:tt) => {{
                let opr = load!($opr) as i8 as u16;
                self.counter += 2;
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
                    self.counter += 1;
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
                    self.counter += 1;
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
                info!("HALT");
            }};
            (STOP) => {{
                self.halting = true;
                info!("STOP");
            }};
            (DI) => {{
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
                self.counter += 1;
            }};
            (JP (HL)) => {{
                self.reg.pc = self.reg.hl();
            }};
            (JP $cc:tt, nn) => {{
                let addr = load!(nn);
                if cond!($cc) {
                    self.reg.pc = addr;
                    self.counter += 1;
                }
            }};
            (JR $opr:tt) => {{
                let r = load!($opr) as u16;
                self.reg.pc = self.reg.pc.wrapping_add(r);
                self.counter += 1;
            }};
            (JR $cc:tt, $opr:tt) => {{
                let r = load!($opr) as u16;
                if cond!($cc) {
                    self.reg.pc = self.reg.pc.wrapping_add(r);
                    self.counter += 1;
                }
            }};
            (CALL $opr:tt) => {{
                let addr = load!($opr);
                self.push_u16(self.reg.pc);
                self.reg.pc = addr;
                self.counter += 1;
            }};
            (CALL $cc:tt, $opr:tt) => {{
                let addr = load!($opr);
                if cond!($cc) {
                    self.push_u16(self.reg.pc);
                    self.reg.pc = addr;
                    self.counter += 1;
                }
            }};
            (RST $opr:expr) => {{
                self.push_u16(self.reg.pc);
                self.reg.pc = $opr;
                self.counter += 1;
            }};

            (RET) => {{
                self.counter += 1;
                self.reg.pc = self.pop_u16();
            }};
            (RET $cc:tt) => {{
                self.counter += 1;
                if cond!($cc) {
                    self.reg.pc = self.pop_u16();
                    self.counter += 1;
                }
            }};
            (RETI) => {{
                self.reg.pc = self.pop_u16();
                self.counter += 1;
                self.interrupt_master_enable = true;
            }};

            (UNK) => {
                todo!("Unknown instruction")
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
                let opc_cb = self.fetch();
                match opc_cb {
                    $( ConstEval::<{$ix}>::VALUE => gen_instr!($mne $opr), )*
                }
            }};
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
        let lo = self.fetch();
        let hi = self.fetch();
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
    fn trace(&mut self, pc: u16, opc: u8) {
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
            "{pc:04X}: {bytes:8} | {asm:16} | \
            A:{a:02X} B:{b:02X} C:{c:02X} D:{d:02X} E:{e:02X} H:{h:02X} L:{l:02X} \
            SP:{sp:04X} F:{zf}{nf}{hf}{cf} IME:{ime} IE:{ie:02X} IF:{inf:02X} CYC:{cyc}",
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
            ie = *self.interrupt_enable.borrow(),
            inf = *self.interrupt_flag.borrow(),
            cyc = self.counter
        );
    }
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
            opr1.and_then(|opr1| opr2.map(|opr2| format!("${:02X}{:02X}", opr2, opr1)))
                .unwrap_or("$????".to_string())
        }};
        ((nn)) => {{
            bytes += 2;
            opr1.and_then(|opr1| opr2.map(|opr2| format!("(${:02X}{:02X})", opr2, opr1)))
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
