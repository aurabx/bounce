#[cfg(test)]
mod tests {
    use crate::query::poller::{init_poller_state, QueryPollerState};
    use tokio::sync::oneshot;

    #[test]
    fn test_init_poller_state_has_no_shutdown_sender() {
        let state = init_poller_state();
        assert!(
            state.shutdown_sender.is_none(),
            "Initial state should have no shutdown sender"
        );
    }

    #[test]
    fn test_poller_state_accepts_shutdown_sender() {
        let mut state = init_poller_state();
        let (tx, _rx) = oneshot::channel::<()>();
        state.shutdown_sender = Some(tx);
        assert!(state.shutdown_sender.is_some());
    }

    #[test]
    fn test_poller_state_take_shutdown_sender() {
        let mut state = init_poller_state();
        let (tx, _rx) = oneshot::channel::<()>();
        state.shutdown_sender = Some(tx);

        let taken = state.shutdown_sender.take();
        assert!(taken.is_some());
        assert!(
            state.shutdown_sender.is_none(),
            "After take, sender should be None"
        );
    }

    #[tokio::test]
    async fn test_poller_shutdown_signal_delivery() {
        let (tx, rx) = oneshot::channel::<()>();

        // Simulate storing and then sending shutdown
        let mut state = QueryPollerState {
            shutdown_sender: Some(tx),
        };

        let sender = state.shutdown_sender.take().unwrap();
        assert!(sender.send(()).is_ok());

        // The receiver should get the signal
        let result = rx.await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_poller_shutdown_receiver_dropped() {
        let (tx, rx) = oneshot::channel::<()>();

        // Drop the receiver first
        drop(rx);

        // Sending should fail because receiver is gone
        let result = tx.send(());
        assert!(result.is_err());
    }
}
