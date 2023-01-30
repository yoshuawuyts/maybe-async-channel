use maybe_async_channel::*;

#[test]
fn sync_call() {
    let (sender, receiver) = bounded::<helpers::NotAsync, usize>(10);
}

#[test]
fn async_call() {
    async {
        let (sender, receiver) = bounded::<helpers::Async, usize>(42);
    };
}
