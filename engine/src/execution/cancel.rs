use std::future::Future;
use std::time::Duration;

pub(crate) async fn await_with_cancel<T, Fut, FCancel>(
    future: Fut,
    should_cancel: &mut FCancel,
) -> Option<T>
where
    Fut: Future<Output = T>,
    FCancel: FnMut() -> bool,
{
    tokio::pin!(future);

    loop {
        if should_cancel() {
            return None;
        }

        tokio::select! {
            output = &mut future => return Some(output),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if should_cancel() {
                    return None;
                }
            }
        }
    }
}
