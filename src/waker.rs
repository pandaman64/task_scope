use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::scope::Cancellation;

pub(crate) struct WakerData {
    pub(crate) cancellation: Cancellation,
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

// returns true if the task is canceled.
pub(crate) fn is_canceled(cx: &mut Context) -> bool {
    unsafe {
        if let Some(data) = retrieve_data(cx) {
            if let Some(sender) = data.cancellation.sender.upgrade() {
                match sender.lock().unwrap().poll_canceled(cx) {
                    Poll::Ready(()) => true,
                    Poll::Pending => false,
                }
            } else {
                true
            }
        } else {
            false
        }
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

fn raw_waker(cancellation: Cancellation, original: Waker) -> RawWaker {
    let data = Box::into_raw(Box::new(WakerData {
        cancellation,
        original,
    }));

    RawWaker::new(data.cast(), &VTABLE)
}

pub(crate) unsafe fn waker(cancellation: Cancellation, original: Waker) -> Waker {
    Waker::from_raw(raw_waker(cancellation, original))
}

unsafe fn clone(data: *const ()) -> RawWaker {
    let data_ref = &*data.cast::<WakerData>();
    raw_waker(data_ref.cancellation.clone(), data_ref.original.clone())
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
