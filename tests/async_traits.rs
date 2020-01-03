use std::time::Duration;
use tokio::io::*;
use tokio::time::timeout;

use task_scope::{cancelable, scope, spawn};

#[test]
fn test_infinite() {
    // this thread never ends as there is a task loops infinitely
    std::thread::spawn(|| {
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            timeout(
                Duration::from_millis(100),
                scope(async {
                    println!("parent started");
                    spawn(async {
                        println!("infinite loop");
                        let mut fast_read = repeat(0);
                        let mut fast_write = sink();

                        copy(&mut fast_read, &mut fast_write).await.unwrap();

                        panic!("never ends");
                    })
                    .await
                    .unwrap();

                    println!("parent end");
                }),
            )
            .await
            .unwrap_err();
        });
    });
}

#[tokio::test(threaded_scheduler)]
async fn test_cancel_infinite() {
    timeout(
        Duration::from_millis(100),
        scope(async {
            println!("parent started");
            spawn(async {
                // we need Box::pin for Unpin bound
                let mut fast_read = Box::pin(cancelable(repeat(0)));
                let mut fast_write = sink();

                copy(&mut fast_read, &mut fast_write).await.unwrap_err();

                println!("copy is canceled");
            })
            .await
            .unwrap();

            println!("parent end");
        }),
    )
    .await
    .unwrap_err();

    println!("canceled");
}
