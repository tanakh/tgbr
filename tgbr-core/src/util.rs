use serde::{Deserialize, Serialize};

macro_rules! trait_alias {
    (pub trait $name:ident = $($traits:tt)+) => {
        pub trait $name: $($traits)* {}
        impl<T: $($traits)*> $name for T {}
    };
}

pub(crate) use trait_alias;

#[derive(Default, Serialize, Deserialize)]
pub struct ClockDivider {
    count: u64,
    period: u64,
}

impl ClockDivider {
    pub fn new() -> Self {
        Self {
            count: 0,
            period: 0,
        }
    }

    pub fn with_period(period: u64) -> Self {
        Self { count: 0, period }
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }

    pub fn set_period(&mut self, period: u64) {
        self.count = 0;
        self.period = period;
    }

    pub fn tick(&mut self) -> bool {
        self.count += 1;
        if self.count >= self.period {
            self.count = 0;
            true
        } else {
            false
        }
    }
}

pub struct ConstEval<const V: u8>;

impl<const V: u8> ConstEval<V> {
    pub const VALUE: u8 = V;
}

macro_rules! pack {
    (@packing $view:ident $x:literal..=$y:literal => $v:expr $(, $($rest:tt)*)?) => {
        $view[$x..=$y].store($v);
        pack!(@packing $view $($($rest)*)*);
    };
    (@packing $view:ident $x:literal => $v:expr $(, $($rest:tt)*)?) => {
        $view.set($x, $v);
        pack!(@packing $view $($($rest)*)*);
    };
    (@packing $view:ident $(,)?) => {};
    (@packing $($rest:tt)*) => {
        compile_error!("Invalid input for macro pack!");
    };
    ($($input:tt)*) => {{
        use bitvec::prelude::*;
        let mut data = 0;
        let view = data.view_bits_mut::<bitvec::prelude::Lsb0>();
        pack!(@packing view $($input)*);
        data
    }};
}

pub(crate) use pack;

#[test]
fn test_pack() {
    let v: u8 = pack! {
        0..=2 => 0b101_u8,
        3 => true,
        4..=6 => 0b100_u8
    };
    assert_eq!(v, 0b01001101);
}

pub fn to_si_bytesize(x: u64) -> String {
    bytesize::ByteSize(x).to_string_as(true)
}
