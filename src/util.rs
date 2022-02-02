use std::{cell::RefCell, ops::Deref, rc::Rc};

pub struct Ref<T: ?Sized>(pub Rc<RefCell<T>>);

impl<T> Ref<T> {
    pub fn new(t: T) -> Self {
        Ref(Rc::new(RefCell::new(t)))
    }
}

impl<T: ?Sized> Deref for Ref<T> {
    type Target = RefCell<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> Clone for Ref<T> {
    fn clone(&self) -> Self {
        Ref(Rc::clone(&self.0))
    }
}

#[derive(Default)]
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

// trait BitView: Sized {
//     fn lsb0(&self) -> BitSlice<Self>;
//     fn lsb0_mut(&mut self) -> BitSliceMut<Self>;
// }

// impl BitView for u8 {
//     fn lsb0(&self) -> BitSlice<Self> {
//         BitSlice { data: self }
//     }

//     fn lsb0_mut(&mut self) -> BitSliceMut<Self> {
//         BitSliceMut { data: self }
//     }
// }

// struct BitSlice<'a, T> {
//     data: &'a T,
// }

// use std::ops::{BitAnd, BitAndAssign, BitOrAssign, Index, Not, Shl};

// impl<T> Index<usize> for BitSlice<'_, T>
// where
//     T: BitAnd<Output = T> + Shl<usize, Output = T> + From<u8> + Eq + Copy,
// {
//     type Output = bool;

//     fn index(&self, index: usize) -> &Self::Output {
//         if *self.data & (T::from(1) << index) != T::from(0) {
//             &true
//         } else {
//             &false
//         }
//     }
// }

// struct BitSliceMut<'a, T> {
//     data: &'a mut T,
// }

// impl<T> BitSliceMut<'_, T>
// where
//     T: BitOrAssign + BitAndAssign + Shl<usize, Output = T> + Not<Output = T> + From<u8>,
// {
//     pub fn set(&mut self, index: usize, value: bool) {
//         if value {
//             *self.data |= T::from(1) << index;
//         } else {
//             *self.data &= !(T::from(1) << index);
//         }
//     }
// }

// #[test]
// fn test_bitview() {
//     let mut data = 0b10101010_u8;
//     let slice = data.lsb0();
//     for i in 0..8 {
//         assert_eq!(slice[i], data & (1 << i) != 0);
//     }
// }
