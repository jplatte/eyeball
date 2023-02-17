use futures_channel::mpsc;

#[derive(Debug)]
pub(crate) struct Notifier<T> {
    senders: Vec<mpsc::UnboundedSender<T>>,
}

impl<T> Notifier<T> {
    pub(crate) const fn new() -> Self {
        Self { senders: Vec::new() }
    }

    pub(crate) fn add_sender(&mut self, sender: mpsc::UnboundedSender<T>) {
        self.senders.push(sender);
    }

    pub(crate) fn notify(&mut self, get_value: impl Fn() -> T) {
        self.senders.retain_mut(move |sender| match sender.unbounded_send(get_value()) {
            Ok(_) => true,
            Err(e) if e.is_disconnected() => false,
            Err(e) => panic!("logic error: {e}"),
        });
    }
}

impl<T> Default for Notifier<T> {
    fn default() -> Self {
        Self::new()
    }
}
