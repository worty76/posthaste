//! Persists text shortly after each keystroke.
//!
//! Writes are serialised on one chain so they reach the database in order,
//! and `cancel()` waits for a write already in flight. That matters at save
//! time: a late draft write landing after the note was saved would bring
//! back a draft of text the user has already filed.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use futures::future::{FutureExt, Shared};
use gloo_timers::callback::Timeout;
use leptos::task::spawn_local;
use posthaste_model::CommandError;

/// How long typing pauses before a draft is written.
const DRAFT_DELAY_MS: u32 = 250;

type Boxed<T> = Pin<Box<dyn Future<Output = T>>>;
/// Every write so far, in order. Cloning it gives a way to wait for them.
type Chain = Shared<Boxed<()>>;

/// The text to store and the category beside it.
type Entry = (String, Option<String>);

type Write = dyn Fn(String, Option<String>) -> Boxed<Result<(), CommandError>>;

#[derive(Clone)]
pub struct DraftSync {
    inner: Rc<Inner>,
}

struct Inner {
    delay_ms: u32,
    write: Box<Write>,
    on_error: Box<dyn Fn(CommandError)>,
    state: RefCell<State>,
}

struct State {
    timer: Option<Timeout>,
    pending: Option<Entry>,
    chain: Chain,
}

impl DraftSync {
    pub fn new<W, F>(write: W, on_error: impl Fn(CommandError) + 'static) -> Self
    where
        W: Fn(String, Option<String>) -> F + 'static,
        F: Future<Output = Result<(), CommandError>> + 'static,
    {
        DraftSync::with_delay(DRAFT_DELAY_MS, write, on_error)
    }

    /// Like `new`, writing after a pause of `delay_ms`.
    pub fn with_delay<W, F>(
        delay_ms: u32,
        write: W,
        on_error: impl Fn(CommandError) + 'static,
    ) -> Self
    where
        W: Fn(String, Option<String>) -> F + 'static,
        F: Future<Output = Result<(), CommandError>> + 'static,
    {
        let settled: Boxed<()> = Box::pin(async {});
        DraftSync {
            inner: Rc::new(Inner {
                delay_ms,
                write: Box::new(move |content, category| Box::pin(write(content, category))),
                on_error: Box::new(on_error),
                state: RefCell::new(State {
                    timer: None,
                    pending: None,
                    chain: settled.shared(),
                }),
            }),
        }
    }

    /// Remembers the latest text and writes it after a short pause.
    pub fn schedule(&self, content: String, category: Option<String>) {
        let this = self.clone();
        let timer = Timeout::new(self.inner.delay_ms, move || {
            // The chain is driven by `flush` itself; nothing to await here.
            drop(this.flush());
        });
        let mut state = self.inner.state.borrow_mut();
        state.pending = Some((content, category));
        // Replacing the timer cancels the one before it.
        state.timer = Some(timer);
    }

    /// Writes any pending text now. The returned future resolves once it,
    /// and every write before it, is stored.
    pub fn flush(&self) -> impl Future<Output = ()> {
        let mut state = self.inner.state.borrow_mut();
        state.timer = None;
        if let Some((content, category)) = state.pending.take() {
            let inner = self.inner.clone();
            let before = state.chain.clone();
            let next: Boxed<()> = Box::pin(async move {
                before.await;
                if let Err(error) = (inner.write)(content, category).await {
                    (inner.on_error)(error);
                }
            });
            state.chain = next.shared();
            // Run it even if nobody awaits the returned future.
            spawn_local(state.chain.clone());
        }
        state.chain.clone()
    }

    /// Drops any pending text. The returned future resolves once writes
    /// already in flight have finished.
    pub fn cancel(&self) -> impl Future<Output = ()> {
        let mut state = self.inner.state.borrow_mut();
        state.timer = None;
        state.pending = None;
        state.chain.clone()
    }
}
