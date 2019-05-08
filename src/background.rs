/// Tiny thread-backed background job thing
///
/// This is very similar to ffi_helper's Task
/// https://github.com/Michael-F-Bryan/ffi_helpers

use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError, RecvTimeoutError};
use std::thread;
use std::panic::{catch_unwind, RefUnwindSafe};

#[derive(Debug, Clone)]
pub struct ControlToken(Arc<ControlTokenInner>);

#[derive(Debug, Default)]
pub struct ControlTokenInner {
    cancel: AtomicBool,
    pause: AtomicBool,
}

impl ControlToken {
    pub fn new() -> Self {
        Self(Arc::new(ControlTokenInner::default()))
    }

    pub fn cancel(&self) {
        self.0.cancel.store(true, Ordering::SeqCst);
    }

    pub fn pause(&self) {
        self.0.pause.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.0.pause.store(false, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::SeqCst)
    }

    pub fn is_paused(&self) -> bool {
        self.0.pause.load(Ordering::SeqCst)
    }

    pub fn is_cancelled_with_pause(&self) -> bool {
        self.is_cancelled() || (self.handle_pause() && self.is_cancelled())
    }

    pub fn handle_pause(&self) -> bool {
        let mut paused = false;

        while self.is_paused() && !self.is_cancelled() {
            paused = true;
            thread::park_timeout(Duration::from_millis(10));
        }

        paused
    }

    pub fn result(&self) -> Result<(), ()> {
        if self.is_cancelled() {
            Err(())
        } else {
            Ok(())
        }
    }
}

impl Default for ControlToken {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BackgroundHandle<T> {
    result: Receiver<std::thread::Result<T>>,
    control: ControlToken
}

impl<T> BackgroundHandle<T> {
    pub fn spawn<K>(task: K) -> BackgroundHandle<T>
    where
        K: Background<Output=T> + RefUnwindSafe + Send + Sync + 'static,
        T: Send + Sync + 'static
    {
        let (tx, rx) = mpsc::channel();
        let control = ControlToken::new();
        let inner_control = control.clone();

        thread::spawn(move || {
            let response = catch_unwind(|| task.run(&inner_control));
            let _ = tx.send(response);
        });

        BackgroundHandle {
            result: rx,
            control
        }
    }

    pub fn poll(&self) -> Option<T> {
        match self.result.try_recv() {
            Ok(value) => Some(value.unwrap()),
            Err(TryRecvError::Empty) => None,
            Err(e) => panic!("{:?}", e)
        }
    }

    pub fn wait_timeout(&self, wait: Duration) -> Option<T> {
        match self.result.recv_timeout(wait) {
            Ok(value) => Some(value.unwrap()),
            Err(RecvTimeoutError::Timeout) => None,
            Err(e) => panic!("{:?}", e)
        }
    }

    pub fn wait(self) -> T {
        match self.result.recv() {
            Ok(value) => value.unwrap(),
            Err(e) => panic!("{:?}", e)
        }
    }

    pub fn cancel(&self) {
        self.control.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.control.is_cancelled()
    }


    pub fn pause(&self) {
        self.control.pause();
    }

    pub fn resume(&self) {
        self.control.resume();
    }

    pub fn is_paused(&self) -> bool {
        self.control.is_paused()
    }
}

impl<T> Drop for BackgroundHandle<T> {
    fn drop(&mut self) {
        self.cancel();
    }
}

pub trait Background: Send + Sync {
    type Output: Send + Sync;

    fn run(&self, control: &ControlToken) -> Self::Output;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[derive(Debug, Clone, Copy)]
    pub struct Tick;

    impl Background for Tick {
        type Output = Result<u32, u32>;

        fn run(&self, control: &ControlToken) -> Self::Output {
            let mut ticks = 0;

            while ticks < 100 && !control.is_cancelled_with_pause() {
                ticks += 1;

                thread::sleep(Duration::from_millis(10));
            }

            control.result().map(|_| ticks).map_err(|_| ticks)
        }
    }

    #[test]
    fn it_cancels() {
        let task = Tick;

        let handle = BackgroundHandle::spawn(task);

        for _ in 0..10 {
            thread::sleep(Duration::from_millis(10));
            let got = handle.poll();
            assert!(got.is_none());
        }

        handle.cancel();

        let ret = handle.wait();
        assert!(ret.is_err());
        let ticks = ret.unwrap_err();
        assert!(9 <= ticks && ticks <= 12);
    }


    #[test]
    fn it_pauses() {
        let task = Tick;

        let handle = BackgroundHandle::spawn(task);

        handle.pause();

        for _ in 0..10 {
            thread::sleep(Duration::from_millis(10));
            let got = handle.poll();
            assert!(got.is_none());
        }

        handle.resume();

        for _ in 0..10 {
            thread::sleep(Duration::from_millis(10));
            let got = handle.poll();
            assert!(got.is_none());
        }

        handle.cancel();

        let ret = handle.wait();
        assert!(ret.is_err());
        let ticks = ret.unwrap_err();
        assert!(9 <= ticks && ticks <= 12);
    }
}
