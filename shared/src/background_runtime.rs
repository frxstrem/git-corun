use std::any::Any;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::thread;

use futures::future;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, oneshot};
use tokio::task::{self, LocalSet};

use crate::type_map::TypeMap;

type AnyError = Box<dyn std::error::Error>;

enum Event {
    Task(Box<dyn FnOnce() -> task::JoinHandle<()> + Send>),
    SetContext(Box<dyn FnOnce() -> Box<dyn Any> + Send>),
}

pub struct BackgroundRuntime {
    thread_handle: thread::JoinHandle<()>,
    sender: mpsc::UnboundedSender<Event>,
}

impl BackgroundRuntime {
    pub fn new() -> BackgroundRuntime {
        let (sender, receiver) = mpsc::unbounded_channel::<Event>();

        let thread_handle = thread::spawn(move || {
            let mut receiver = receiver;

            let mut rt = Runtime::new().unwrap();
            let local = LocalSet::new();

            CONTEXT_MAP.with(|map| {
                *map.borrow_mut() = Some(TypeMap::new());
            });

            local.block_on(&mut rt, async move {
                let mut task_handles = Vec::new();

                while let Some(event) = receiver.recv().await {
                    match event {
                        Event::Task(task) => {
                            let handle = task();
                            task_handles.push(handle);
                        }
                        Event::SetContext(func) => {
                            CONTEXT_MAP.with(|map| {
                                let mut map = map.borrow_mut();
                                let map = map.as_mut().unwrap();
                                map.insert_any(func())
                            });
                        }
                    }
                }

                // wait for all tasks to complete
                future::join_all(task_handles).await;
            });
        });

        BackgroundRuntime {
            thread_handle,
            sender,
        }
    }

    pub fn add_context<T>(&self, value: T)
    where
        T: Any + Send + 'static,
    {
        // ignore errors
        let _ = self
            .sender
            .send(Event::SetContext(Box::new(move || Box::new(value))));
    }

    pub fn add_context_with<G, T>(&self, func: G)
    where
        G: FnOnce() -> T + Send + 'static,
        T: Any + Send + 'static,
    {
        // ignore errors
        let _ = self
            .sender
            .send(Event::SetContext(Box::new(move || Box::new(func()))));
    }

    pub fn finish(self) -> Result<(), AnyError> {
        drop(self.sender);
        self.thread_handle.join().unwrap(); // TODO: handle panics in thread
        Ok(())
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawn_with(move || future)
    }

    pub fn spawn_with<G, F>(&self, func: G) -> JoinHandle<F::Output>
    where
        G: FnOnce() -> F + Send + 'static,
        F: Future + 'static,
        F::Output: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();

        let func: Box<dyn FnOnce() -> task::JoinHandle<()> + Send> = Box::new(move || {
            task::spawn_local(async move {
                let result = func().await;
                let _ = sender.send(result);
            })
        });

        let _ = self.sender.send(Event::Task(func));

        JoinHandle { receiver }
    }
}

thread_local! {
    static CONTEXT_MAP: RefCell<Option<TypeMap>> = RefCell::new(None);
}

pub fn get_context<T: Any>() -> Option<Rc<T>> {
    CONTEXT_MAP.with(|map| {
        let map = map.borrow();
        let map = map
            .as_ref()
            .expect("Calling get_context outside of BackgroundRuntime");
        map.get::<T>()
    })
}

pub struct JoinHandle<T> {
    receiver: oneshot::Receiver<T>,
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<T, JoinError>> {
        Pin::new(&mut self.receiver)
            .poll(cx)
            .map_err(|_| JoinError { _private: () })
    }
}

pub struct JoinError {
    _private: (),
}

#[cfg(test)]
mod test {
    use super::*;

    use std::time::Duration;

    use tokio::time::delay_for;

    #[tokio::test]
    async fn background_runtime() {
        let runtime = BackgroundRuntime::new();
        runtime.add_context::<i32>(123);

        let mut results = Vec::new();

        {
            results.push(runtime.spawn(async move {
                delay_for(Duration::from_millis(300)).await;
                true
            }));
        }

        {
            results.push(runtime.spawn_with(move || {
                let ctx = get_context::<i32>().unwrap();

                async move {
                    delay_for(Duration::from_millis(400)).await;

                    assert_eq!(*ctx, 123);

                    true
                }
            }));
        }

        runtime.finish().unwrap();

        assert_eq!(
            true,
            future::join_all(results)
                .await
                .into_iter()
                .all(|x| x.unwrap_or(false))
        );
    }
}
