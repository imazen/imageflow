# Async roadmap

Currently very few components support end-to-end async.  Lang progress has been years slower than expected.

* Write bidirectional adapters for C# <-> Rust features/streams, and benchmark

For a foreign lang to provide a future, it needs to

* poll() with two pointers, one pinned reference to its state, another for the wake context.
* support drop(), which cancels the task
* provide drop methods for data and error
* never throw foreign exceptions


To send the wake context to a different thread, it needs to call get_threadsafe_wake_context(wake_context). For futures 0.1, this will Box::new(task::current()), but for 0.3 it will just be another reference (pointer)


` fn poll(pinned pointer to self, wake_context pointer) -> NotReady, Ready(Some), Error()
` drop (cancellation)`




## Change summary 0.1 -> 0.3

Old Poll:

```
// From https://github.com/rust-lang-nursery/futures-rs/blob/0.1/src/poll.rs#L20:10
/// Return type of the `Future::poll` method, indicates whether a future's value is ready or not.
/// * `Ok(Async::Ready(t))` means that a future has successfully resolved
/// * `Ok(Async::NotReady)` means that a future is not ready to complete yet
/// * `Err(e)` means that a future has completed with the given failure
pub type Poll<T, E> = Result<Async<T>, E>;
```


New  poll doesn't embed Result:

```
// From https://doc.rust-lang.org/nightly/src/core/task/poll.rs.html#21-31
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Poll<T> {
    /// Represents that a value is immediately ready.
    Ready(T),

    /// Represents that a value is not ready yet.
    ///
    /// When a function returns `Pending`, the function *must* also
    /// ensure that the current task is scheduled to be awoken when
    /// progress can be made.
    Pending,
}
```


Old future:

```
pub trait Future {
    type Item;
    type Error;

    /// Query this future to see if its value has become available, registering
    /// interest if it is not.
    ///
    /// This function will check the internal state of the future and assess
    /// whether the value is ready to be produced. Implementers of this function
    /// should ensure that a call to this **never blocks** as event loops may
    /// not work properly otherwise.
    ///
    /// When a future is not ready yet, the `Async::NotReady` value will be
    /// returned. In this situation the future will *also* register interest of
    /// the current task in the value being produced. This is done by calling
    /// `task::park` to retrieve a handle to the current `Task`. When the future
    /// is then ready to make progress (e.g. it should be `poll`ed again) the
    /// `unpark` method is called on the `Task`.
    ///
    /// More information about the details of `poll` and the nitty-gritty of
    /// tasks can be [found online at tokio.rs][poll-dox].
    ///
    /// [poll-dox]: https://tokio.rs/docs/going-deeper-futures/futures-model/
    ///
    fn poll(&mut self) -> Poll<Self::Item, Self::Error>;
 ```

Futures now take two pointers instead of one, rather than relying on task::park
The second is thread-local but can be sent after calling .into_waker()

* futures 0.1 - `fn poll(&mut self) -> Poll<Self::Item, Self::Error>`

* [futures std](https://doc.rust-lang.org/nightly/std/future/trait.Future.html) - `fn poll(self: Pin<&mut Self>, lw: &LocalWaker) -> Poll<Self::Output>`

Streams in 0.1 are like futures with Option<>, where None indicates end of stream

`fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error>`



## Current stable Stream interface (futures 0.1)

type Item The type of item this stream will yield on success.
type Error The type of error this stream may generate.

### Required Methods
``` fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error>```

Attempt to pull out the next value of this stream, returning None if the stream is finished.

This method, like Future::poll, is the sole method of pulling out a value from a stream. This method must also be
run within the context of a task typically and implementors of this trait must ensure that implementations of this
method do not block, as it may cause consumers to behave badly.

### Return value
If NotReady is returned then this stream's next value is not ready yet and implementations will ensure that the
current task will be notified when the next value may be ready. If Some is returned then the returned value
represents the next value on the stream. Err indicates an error happened, while Ok indicates whether there was
a new item on the stream or whether the stream has terminated.

### Panics
Once a stream is finished, that is Ready(None) has been returned, further calls to poll may result in a
panic or other "bad behavior". If this is difficult to guard against then the fuse adapter can be used
to ensure that poll always has well-defined semantics.

# Not-yet-usable futures draft in rust nightly (futures 0.3)

# https://rust-lang-nursery.github.io/futures-rs/blog/2018/07/19/futures-0.3.0-alpha.1.html
# https://github.com/rust-lang-nursery/futures-rs/blob/05f5e3cd21e47ae290a47c326214c5b859ffdc90/futures-core/src/stream/mod.rs

```
    /// Attempt to pull out the next value of this stream, registering the
    /// current task for wakeup if the value is not yet available, and returning
    /// `None` if the stream is exhausted.
    ///
    /// # Return value
    ///
    /// There are several possible return values, each indicating a distinct
    /// stream state:
    ///
    /// - `Poll::Pending` means that this stream's next value is not ready
    /// yet. Implementations will ensure that the current task will be notified
    /// when the next value may be ready.
    ///
    /// - `Poll::Ready(Some(val))` means that the stream has successfully
    /// produced a value, `val`, and may produce further values on subsequent
    /// `poll_next` calls.
    ///
    /// - `Poll::Ready(None)` means that the stream has terminated, and
    /// `poll_next` should not be invoked again.
    ///
    /// # Panics
    ///
    /// Once a stream is finished, i.e. `Ready(None)` has been returned, further
    /// calls to `poll_next` may result in a panic or other "bad behavior".  If
    /// this is difficult to guard against then the `fuse` adapter can be used
    /// to ensure that `poll_next` always returns `Ready(None)` in subsequent
    /// calls.
    fn poll_next(
        self: Pin<&mut Self>,
        lw: &LocalWaker,
    ) -> Poll<Option<Self::Item>>;
 ```
