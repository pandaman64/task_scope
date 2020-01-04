use std::time::Duration;
use tokio::time::{delay_for, timeout};

use task_scope::{scope, spawn};

#[tokio::test]
async fn test_basic() {
    scope(async {
        spawn(async {
            delay_for(Duration::from_millis(500)).await;

            println!("child is done");
        })
        .await;
        println!("parent is done");
    })
    .await;

    delay_for(Duration::from_millis(1000)).await;
}

#[tokio::test]
async fn test_drop() {
    timeout(
        Duration::from_millis(50),
        scope(async {
            spawn(async {
                println!("child started");
                delay_for(Duration::from_millis(100)).await;

                panic!("child is canceled");
            })
            .await;
        }),
    )
    .await
    .unwrap_err();
}
