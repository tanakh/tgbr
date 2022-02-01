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

pub struct ConstEval<const V: u8>;

impl<const V: u8> ConstEval<V> {
    pub const VALUE: u8 = V;
}
