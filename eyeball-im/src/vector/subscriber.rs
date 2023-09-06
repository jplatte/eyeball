use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use tokio::sync::broadcast::{
    error::{RecvError, TryRecvError},
    Receiver,
};
use tokio_util::sync::ReusableBoxFuture;
#[cfg(feature = "tracing")]
use tracing::info;

use super::{BroadcastMessage, VectorDiff};

/// A subscriber for updates of a [`Vector`].
///
/// Use its [`Stream`] implementation to interact with it (futures-util and
/// other futures-related crates have extension traits with convenience
/// methods).
#[derive(Debug)]
pub struct VectorSubscriber<T> {
    inner: ReusableBoxFuture<'static, SubscriberFutureReturn<BroadcastMessage<T>>>,
}

impl<T: Clone + Send + Sync + 'static> VectorSubscriber<T> {
    pub(super) fn new(rx: Receiver<BroadcastMessage<T>>) -> Self {
        Self { inner: ReusableBoxFuture::new(make_future(rx)) }
    }
}

impl<T: Clone + Send + Sync + 'static> Stream for VectorSubscriber<T> {
    type Item = VectorDiff<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (result, mut rx) = ready!(self.inner.poll(cx));

        let poll = match result {
            Ok(msg) => Poll::Ready(Some(msg.diff)),
            Err(RecvError::Closed) => Poll::Ready(None),
            Err(RecvError::Lagged(_)) => {
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
                            break Poll::Ready(None);
                        }
                        // Lagged twice in a row, is this possible? If it is, it's fine to just
                        // loop again and look at the next try_recv result.
                        Err(TryRecvError::Lagged(_)) => {}
                        Err(TryRecvError::Empty) => match msg {
                            // We exhausted the internal buffer using try_recv, msg contains the
                            // last message from it, which we use for the reset.
                            Some(msg) => {
                                break Poll::Ready(Some(VectorDiff::Reset { values: msg.state }));
                            }
                            // We exhausted the internal buffer using try_recv but there was no
                            // message in it, even though we got TryRecvError::Lagged(_) before.
                            None => unreachable!("got no new message via try_recv after lag"),
                        },
                    }
                }
            }
        };

        self.inner.set(make_future(rx));
        poll
    }
}

type SubscriberFutureReturn<T> = (Result<T, RecvError>, Receiver<T>);

async fn make_future<T: Clone>(mut rx: Receiver<T>) -> SubscriberFutureReturn<T> {
    let result = rx.recv().await;
    (result, rx)
}
