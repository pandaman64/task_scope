use futures_intrusive::channel::shared::state_broadcast_channel;
use futures_intrusive::sync::Semaphore;

use std::future::Future;
use std::sync::Arc;

use crate::with_token::WithToken;
use crate::{Join, Token};

fn make_join() -> (Arc<Join>, Arc<Semaphore>) {
    let sem = Arc::new(Semaphore::new(true, 1));

    let mut releaser = sem.try_acquire(1).unwrap();
    assert_eq!(releaser.disarm(), 1);
    drop(releaser);

    (Arc::new(Join { sem: sem.clone() }), sem)
}

pub async fn scope<Fut>(inner: Fut) -> Fut::Output
where
    Fut: Future,
{
    let (join, sem) = make_join();

    // signals receivers when this future is canceled (dropped)
    let (_sender, receiver) = state_broadcast_channel();
    let cancel = Arc::new(receiver);

    let inner = WithToken::new(inner);
    futures::pin_mut!(inner);

    let output = futures::future::poll_fn(|cx| {
        let token = Token {
            cancel: cancel.clone(),
            join: Arc::downgrade(&join),
        };

        inner.as_mut().poll(cx, token)
    })
    .await;

    drop(join);
    drop(cancel);

    sem.acquire(1).await;

    output
}
