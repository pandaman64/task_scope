use futures::future::FutureExt;
use futures::{pin_mut, select};
use std::time::Duration;
use tokio::signal::ctrl_c;
use tokio::time::delay_for;

use task_scope::{cancelable, scope, spawn, Canceled};

#[tokio::main]
async fn main() {
    let scope = scope(async {
        // nesting scopes. if you gracefully cancel outer scope, cancellation will be propagated to inner scopes
        spawn(scope(async {
            println!("doing a heavy task");

            let result = cancelable(delay_for(Duration::from_secs(5))).await;

            match result {
                Err(Canceled::Graceful) => {
                    println!("graceful cancellation is requested");

                    // this scope will wait until cancellation finishes
                    delay_for(Duration::from_secs(1)).await;

                    println!("cancellation is done");
                }
                Err(Canceled::Forced) => {
                    // now, the task is in "mercy" period. we should exit immediately
                    println!("forced cancellation");
                }
                Ok(_) => println!("task is done"),
            }
        }))
        .await;

        42
    });
    pin_mut!(scope);

    let mut canceled = false;

    loop {
        select! {
            () = ctrl_c().fuse().map(Result::unwrap) => if canceled {
                println!("force quit");
                break;
            } else {
                println!("canceling... (Ctrl-C to force quit)");
                scope.as_mut().cancel();
                canceled = true;
            },
            result = scope.as_mut() => {
                match result {
                    Ok(v) => println!("scope finished: {}", v),
                    Err(e) => println!("scope canceled: {:?}", e),
                }
                break;
            },
        }
    }
}
