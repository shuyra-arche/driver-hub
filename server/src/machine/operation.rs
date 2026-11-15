use hub_api::{Offer, Release, Reply, Reserve, Withdraw};
use kernel_api::model::identity::TaskId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Intent {
    Offer(Offer),
    Withdraw(Withdraw),
    Reserve(Reserve),
    Release(Release),
}

pub(super) struct Operation {
    pub caller: TaskId,
    pub id: u64,
    pub intent: Intent,
    pub result: Reply,
}
