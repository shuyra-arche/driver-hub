use super::{Command, Control, OperationId, Response};
use kernel_api::model::{
    identity::{PendId, TaskId},
    wait::Wait,
};
use system_api::control::instance::operations as op;
use system_client::{Cleanup, Context};
use weave::{Codec, Contract, Operation};

#[derive(Debug)]
pub enum NativeError {
    Begin(system_client::Error),
    Call(system_client::Error),
    Cleanup(system_client::CleanupError),
    WrongPeer,
    InvalidCaller,
}

/// Real caller-owned Instance Client. Every call's cleanup remains owned here
/// until explicit finish succeeds. Calls may commit before any returned error.
#[must_use = "finish retained reply cleanup before discarding the Native client"]
pub struct Native<F> {
    client: system_client::instance::Client,
    peer: TaskId,
    caller: TaskId,
    operations: F,
    last: u64,
    cleanup: Option<Cleanup>,
}

impl<F: FnMut() -> Option<OperationId>> Native<F> {
    /// # Safety
    /// `entry` must be the actual Instance endpoint delivered by trusted System
    /// bootstrap/Supply, and `peer` must be that trusted System's identity.
    /// SDK binding checks native ownership/rights but does not establish the
    /// service role from an arbitrary Task number, Mark or remote description.
    /// `caller` must be the actual current Task, proven by its native Caller
    /// inspection. Keep this adapter and context in that Task.
    /// `operations` must allocate non-reusing IDs shared by every Instance
    /// client in this actual caller Task for `peer`, including Publish clients.
    /// It must retain its high watermark across rebuilding this adapter. An ID
    /// returned here must not have been used for another intent. Context must
    /// use the same genuine Nest/request allocator as the Task's other users.
    pub unsafe fn bind(
        entry: PendId,
        peer: TaskId,
        caller: TaskId,
        license: PendId,
        context: Context,
        operations: F,
    ) -> Result<Self, NativeError> {
        if caller.get() == 0 {
            return Err(NativeError::InvalidCaller);
        }
        let client = system_client::instance::Client::bind(entry, peer, license, context)
            .map_err(NativeError::Call)?;
        Ok(Self {
            client,
            peer,
            caller,
            operations,
            last: 0,
            cleanup: None,
        })
    }

    /// Retry on failure. Cleanup does not cancel submitted System work.
    pub fn finish(&mut self) -> Result<(), NativeError> {
        if let Some(cleanup) = self.cleanup.as_mut() {
            cleanup.finish().map_err(NativeError::Cleanup)?;
            self.cleanup = None;
        }
        Ok(())
    }

    pub fn is_finished(&self) -> bool {
        self.cleanup.is_none()
    }

    fn call<O>(
        &mut self,
        request: O::Request,
        wait: Wait,
    ) -> Result<<<O as Contract>::Response as Codec>::Decoded, NativeError>
    where
        O: Operation<Interface = op::Interface>,
    {
        self.finish()?;
        let mut pending = match self.client.begin::<O>(request, wait) {
            Ok(pending) => pending,
            Err(failure) => {
                self.cleanup = Some(failure.cleanup);
                return Err(NativeError::Begin(failure.error));
            }
        };
        let result = pending.wait();
        self.cleanup = Some(pending.into_cleanup());
        match result {
            Ok(response) if response.metadata == self.peer => Ok(response.value),
            Ok(_) => Err(NativeError::WrongPeer),
            Err(error) => Err(NativeError::Call(error)),
        }
    }
}

// SAFETY: the SDK checks the endpoint peer and native reply sender and the
// expected operation branch. Construction requires one caller-wide operation
// namespace; this adapter additionally rejects zero/backward/reused IDs locally.
// Every Pending/Failure transfers Cleanup here before any result is returned.
unsafe impl<F: FnMut() -> Option<OperationId>> Control for Native<F> {
    type Error = NativeError;
    fn peer(&self) -> system_api::TaskId {
        self.peer.into()
    }
    fn caller(&self) -> system_api::TaskId {
        self.caller.into()
    }
    fn allocate(&mut self) -> Option<OperationId> {
        let id = (self.operations)()?;
        if id.get() <= self.last {
            return None;
        }
        self.last = id.get();
        Some(id)
    }
    fn execute(&mut self, command: Command, wait: Wait) -> Result<Response, NativeError> {
        match command {
            Command::Create(request) => self
                .call::<op::Create>(request, wait)
                .map(Response::Receipt),
            Command::Start(request) => self.call::<op::Start>(request, wait).map(Response::Receipt),
            Command::Stop(request) => self.call::<op::Stop>(request, wait).map(Response::Receipt),
            Command::Retire(request) => self
                .call::<op::Retire>(request, wait)
                .map(Response::Receipt),
            Command::Inspect(request) => self
                .call::<op::Inspect>(request, wait)
                .map(Response::Observation),
            Command::InspectOperation(request) => self
                .call::<op::InspectOperation>(request, wait)
                .map(Response::Receipt),
            Command::Cancel(request) => self
                .call::<op::Cancel>(request, wait)
                .map(Response::Receipt),
        }
    }
    fn may_have_submitted(&self, error: &NativeError) -> bool {
        // Cleanup and SDK setup failures precede submission. A non-Busy Push
        // error and every post-submission wait failure remain conservative.
        match error {
            NativeError::Cleanup(_) => false,
            NativeError::Begin(system_client::Error::Send(_)) => true,
            NativeError::Begin(_) => false,
            _ => true,
        }
    }
}
