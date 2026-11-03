use env::{Mark, PieToken, TaskId};

#[derive(Clone, Copy)]
pub(super) struct Native {
    pub(super) token: PieToken,
    pub(super) giver: TaskId,
    pub(super) owner: TaskId,
    pub(super) mark: Mark,
    pub(super) live: bool,
    pub(super) hole: bool,
}
