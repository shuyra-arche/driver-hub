use env::PieToken;

use super::{alive, Fail, Source};

pub struct Hole(PieToken);

impl Hole {
    pub fn from_raw(token: PieToken) -> Self {
        Self(token)
    }
    pub fn push(&self, _: &[u8], _: env::Wait) -> Result<(), Fail> {
        if !alive(self.0) {
            return Err(Fail { source: Source });
        }
        Ok(())
    }
}
