pub trait LinkCable {
    fn send(&mut self, data: u8);
    fn try_recv(&mut self) -> Option<u8>;
}
