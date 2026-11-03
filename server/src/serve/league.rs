use system_client::identity::{Face, Organization, Query};

pub(super) struct League {
    pub(super) query: Query,
    pub(super) organization: Organization,
    pub(super) activation: Face,
}
