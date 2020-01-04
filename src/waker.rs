use futures::future::OptionFuture;
use futures_intrusive::channel::{shared::StateReceiveFuture, StateId};

use std::task::{Context, RawWaker, RawWakerVTable, Waker};

use crate::Token;

pub(crate) struct WakerData {
    pub(crate) token: Token,
    original: Waker,
}

// to implement safely, need https://github.com/rust-lang/rfcs/issues/2746
// hope this works
pub(crate) unsafe fn retrieve_data<'c, 'w>(cx: &'c mut Context<'w>) -> Option<&'w WakerData> {
    let waker_ptr: *const Waker = cx.waker();
    let data_ptr: *const *const WakerData = waker_ptr.cast();
    let vtable_ptr: *const &'static RawWakerVTable = data_ptr
        .cast::<u8>()
        .add(std::mem::size_of::<*const ()>())
        .cast();
    let vtable_ref = std::ptr::read(vtable_ptr);

    if vtable_ref != &VTABLE {
        // polled from outside of a scope
        // cancellation is not supported
        None
    } else {
        let data_ref = &**data_ptr;
        Some(data_ref)
    }
}

// returns a future that resolves to
// - Some(Some(state_id, true)) when the current scope requests cancellation
// - Some(None) when the current scope is dropped
// - None when the current context doesn't support cancellation
pub(crate) fn cancellation<'c, 'w>(
    cx: &'c mut Context<'w>,
    state_id: StateId,
) -> OptionFuture<StateReceiveFuture<parking_lot::RawMutex, bool>> {
    unsafe {
        if let Some(data) = retrieve_data(cx) {
            Some(data.token.cancel.receive(state_id)).into()
        } else {
            None.into()
        }
    }
}

pub(crate) unsafe fn waker(token: Token, original: Waker) -> Waker {
    Waker::from_raw(raw_waker(token, original))
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

fn raw_waker(token: Token, original: Waker) -> RawWaker {
    let data = Box::into_raw(Box::new(WakerData { token, original }));

    RawWaker::new(data.cast(), &VTABLE)
}

unsafe fn clone(data: *const ()) -> RawWaker {
    let data_ref = &*data.cast::<WakerData>();
    raw_waker(data_ref.token.clone(), data_ref.original.clone())
}

unsafe fn wake(data: *const ()) {
    let data = Box::from_raw(data.cast::<WakerData>() as *mut WakerData);
    data.original.wake();
}

unsafe fn wake_by_ref(data: *const ()) {
    let data_ref = &*data.cast::<WakerData>();
    data_ref.original.wake_by_ref();
}

unsafe fn drop(data: *const ()) {
    Box::from_raw(data.cast::<WakerData>() as *mut WakerData);
}
