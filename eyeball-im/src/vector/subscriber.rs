use std::{
    fmt,
    hint::unreachable_unchecked,
    mem,
    pin::Pin,
    task::{ready, Context, Poll},
    vec,
};

use crate::reusable_box::ReusableBoxFuture;
use futures_core::Stream;
use imbl::Vector;
use tokio::sync::broadcast::{
    self,
    error::{RecvError, TryRecvError},
    Receiver,
};
#[cfg(feature = "tracing")]
use tracing::info;

use super::{BroadcastMessage, OneOrManyDiffs, VectorDiff};

/// A subscriber for updates of a [`Vector`].
#[derive(Debug)]
pub struct VectorSubscriber<T> {
    values: Vector<T>,
    rx: Receiver<BroadcastMessage<T>>,
}

impl<T: Clone + Send + Sync + 'static> VectorSubscriber<T> {
    pub(super) fn new(items: Vector<T>, rx: Receiver<BroadcastMessage<T>>) -> Self {
        Self { values: items, rx }
    }

    /// Get the items the [`ObservableVector`][super::ObservableVector]
    /// contained when this subscriber was created.
    pub fn values(&self) -> Vector<T> {
        self.values.clone()
    }

    /// Turn this `VectorSubcriber` into a stream of `VectorDiff`s.
    pub fn into_stream(self) -> VectorSubscriberStream<T> {
        VectorSubscriberStream::new(ReusableBoxRecvFuture::new(self.rx))
    }

    /// Turn this `VectorSubcriber` into a stream of `Vec<VectorDiff>`s.
    pub fn into_batched_stream(self) -> VectorSubscriberBatchedStream<T> {
        VectorSubscriberBatchedStream::new(ReusableBoxRecvFuture::new(self.rx))
    }

    /// Destructure this `VectorSubscriber` into the initial values and a stream
    /// of `VectorDiff`s.
    ///
    /// Semantically equivalent to calling `.values()` and `.into_stream()`
    /// separately, but guarantees that the values are not unnecessarily cloned.
    pub fn into_values_and_stream(self) -> (Vector<T>, VectorSubscriberStream<T>) {
        let Self { values, rx } = self;
        (values, VectorSubscriberStream::new(ReusableBoxRecvFuture::new(rx)))
    }

    /// Destructure this `VectorSubscriber` into the initial values and a stream
    /// of `Vec<VectorDiff>`s.
    ///
    /// Semantically equivalent to calling `.values()` and
    /// `.into_batched_stream()` separately, but guarantees that the values
    /// are not unnecessarily cloned.
    pub fn into_values_and_batched_stream(self) -> (Vector<T>, VectorSubscriberBatchedStream<T>) {
        let Self { values, rx } = self;
        (values, VectorSubscriberBatchedStream::new(ReusableBoxRecvFuture::new(rx)))
    }
}

/// A stream of `VectorDiff`s created from a [`VectorSubscriber`].
///
/// Use its [`Stream`] implementation to interact with it (futures-util and
/// other futures-related crates have extension traits with convenience
/// methods).
#[derive(Debug)]
pub struct VectorSubscriberStream<T> {
    inner: ReusableBoxRecvFuture<T>,
    state: VectorSubscriberStreamState<T>,
}

impl<T> VectorSubscriberStream<T> {
    fn new(inner: ReusableBoxRecvFuture<T>) -> Self {
        Self { inner, state: VectorSubscriberStreamState::Recv }
    }
}

#[derive(Debug)]
enum VectorSubscriberStreamState<T> {
    // Stream is waiting on a new message from the inner broadcast receiver.
    Recv,
    // Stream is yielding remaining items from a previous message with multiple
    // diffs.
    YieldBatch { iter: vec::IntoIter<VectorDiff<T>>, rx: Receiver<BroadcastMessage<T>> },
}

// Not clear why this explicit impl is needed, but it's not unsafe so it is fine
impl<T> Unpin for VectorSubscriberStreamState<T> {}

impl<T: Clone + Send + Sync + 'static> Stream for VectorSubscriberStream<T> {
    type Item = VectorDiff<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.state {
            VectorSubscriberStreamState::Recv => {
                let (result, mut rx) = ready!(self.inner.poll(cx));

                let poll = match result {
                    Ok(msg) => match msg.diffs {
                        OneOrManyDiffs::One(diff) => Poll::Ready(Some(diff)),
                        OneOrManyDiffs::Many(diffs) if diffs.is_empty() => {
                            unreachable!("ObservableVectorTransaction never sends empty diffs")
                        }
                        OneOrManyDiffs::Many(mut diffs) if diffs.len() == 1 => {
                            Poll::Ready(Some(diffs.pop().unwrap()))
                        }
                        OneOrManyDiffs::Many(diffs) => {
                            let mut iter = diffs.into_iter();
                            let fst = iter.next().unwrap();
                            self.state = VectorSubscriberStreamState::YieldBatch { iter, rx };
                            return Poll::Ready(Some(fst));
                        }
                    },
                    Err(RecvError::Closed) => Poll::Ready(None),
                    Err(RecvError::Lagged(_)) => {
                        Poll::Ready(handle_lag(&mut rx).map(|values| VectorDiff::Reset { values }))
                    }
                };

                self.inner.set(rx);
                poll
            }
            VectorSubscriberStreamState::YieldBatch { iter, .. } => {
                let diff =
                    iter.next().expect("YieldBatch is never left empty when exiting poll_next");

                if iter.len() == 0 {
                    let old_state =
                        mem::replace(&mut self.state, VectorSubscriberStreamState::Recv);
                    let rx = match old_state {
                        VectorSubscriberStreamState::YieldBatch { rx, .. } => rx,
                        // Safety: We would not be in the outer branch otherwise
                        _ => unsafe { unreachable_unchecked() },
                    };

                    self.inner.set(rx);
                }

                Poll::Ready(Some(diff))
            }
        }
    }
}

/// A batched stream of `VectorDiff`s created from a [`VectorSubscriber`].
///
/// Use its [`Stream`] implementation to interact with it (futures-util and
/// other futures-related crates have extension traits with convenience
/// methods).
#[derive(Debug)]
pub struct VectorSubscriberBatchedStream<T> {
    inner: ReusableBoxRecvFuture<T>,
}

impl<T> VectorSubscriberBatchedStream<T> {
    fn new(inner: ReusableBoxRecvFuture<T>) -> Self {
        Self { inner }
    }
}

impl<T: Clone + Send + Sync + 'static> Stream for VectorSubscriberBatchedStream<T> {
    type Item = Vec<VectorDiff<T>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        fn append<T>(target: &mut Vec<VectorDiff<T>>, source: OneOrManyDiffs<T>) {
            match source {
                OneOrManyDiffs::One(diff) => target.push(diff),
                OneOrManyDiffs::Many(mut diffs) => target.append(&mut diffs),
            }
        }

        let (result, mut rx) = ready!(self.inner.poll(cx));

        let poll = match result {
            Ok(msg) => {
                let mut batch = msg.diffs.into_vec();
                loop {
                    match rx.try_recv() {
                        Ok(msg) => append(&mut batch, msg.diffs),
                        Err(TryRecvError::Empty | TryRecvError::Closed) => {
                            break Poll::Ready(Some(batch));
                        }
                        Err(TryRecvError::Lagged(_)) => {
                            break Poll::Ready(
                                handle_lag(&mut rx)
                                    .map(|values| vec![VectorDiff::Reset { values }]),
                            );
                        }
                    }
                }
            }
            Err(RecvError::Closed) => Poll::Ready(None),
            Err(RecvError::Lagged(_)) => {
                Poll::Ready(handle_lag(&mut rx).map(|values| vec![VectorDiff::Reset { values }]))
            }
        };

        self.inner.set(rx);
        poll
    }
}

fn handle_lag<T: Clone + Send + Sync + 'static>(
    rx: &mut Receiver<BroadcastMessage<T>>,
) -> Option<Vector<T>> {
    let mut msg = None;
    loop {
        match rx.try_recv() {
            // There's a newer message in the receiver's buffer, use that for reset.
            Ok(m) => {
                msg = Some(m);
            }
            // Ideally we'd return a `VecDiff::Reset` with the last state before the
            // channel was closed here, but we have no way of obtaining the last state.
            Err(TryRecvError::Closed) => {
                #[cfg(feature = "tracing")]
                info!("Channel closed after lag, can't return last state");
                return None;
            }
            // Lagged twice in a row, is this possible? If it is, it's fine to just
            // loop again and look at the next try_recv result.
            Err(TryRecvError::Lagged(_)) => {}
            Err(TryRecvError::Empty) => match msg {
                // We exhausted the internal buffer using try_recv, msg contains the
                // last message from it, which we use for the reset.
                Some(msg) => return Some(msg.state),
                // We exhausted the internal buffer using try_recv but there was no
                // message in it, even though we got TryRecvError::Lagged(_) before.
                None => unreachable!("got no new message via try_recv after lag"),
            },
        }
    }
}

type SubscriberFutureReturn<T> = (Result<T, RecvError>, Receiver<T>);

struct ReusableBoxRecvFuture<T> {
    inner: ReusableBoxFuture<'static, SubscriberFutureReturn<BroadcastMessage<T>>>,
}

async fn make_recv_future<T: Clone>(mut rx: Receiver<T>) -> SubscriberFutureReturn<T> {
    let result = rx.recv().await;
    (result, rx)
}

impl<T> ReusableBoxRecvFuture<T>
where
    T: Clone + 'static,
{
    fn set(&mut self, rx: Receiver<BroadcastMessage<T>>) {
        self.inner.set(make_recv_future(rx));
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<SubscriberFutureReturn<BroadcastMessage<T>>> {
        self.inner.poll(cx)
    }
}

impl<T> ReusableBoxRecvFuture<T>
where
    T: Clone + 'static,
{
    fn new(rx: Receiver<BroadcastMessage<T>>) -> Self {
        Self { inner: ReusableBoxFuture::new(make_recv_future(rx)) }
    }
}

fn assert_send<T: Send>(_val: T) {}
#[allow(unused)]
fn assert_make_future_send() {
    #[derive(Clone)]
    struct IsSend(*mut ());
    unsafe impl Send for IsSend {}

    let (_sender, receiver): (_, Receiver<IsSend>) = broadcast::channel(1);

    assert_send(make_recv_future(receiver));
}
// SAFETY: make_future is Send if T is, as proven by assert_make_future_send.
unsafe impl<T: Send> Send for ReusableBoxRecvFuture<T> {}

impl<T> fmt::Debug for ReusableBoxRecvFuture<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReusableBoxRecvFuture").finish()
    }
}
