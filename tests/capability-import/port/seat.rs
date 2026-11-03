use env::PieToken;

pub struct Seat(pub(super) PieToken);

impl Seat {
    pub fn seed(&self) -> PieToken {
        self.0
    }
}
