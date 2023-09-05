/*
 * Created on Thu Aug 10 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

#[doc(hidden)]
pub mod __private;

use std::{
    fmt::Debug,
    future::Future,
    mem,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use higher_kinded_types::ForLifetime;
use parking_lot::Mutex;

use pin_list::{id::Unchecked, CursorMut};
use unique::Unique;

#[macro_export]
macro_rules! EventSource {
    ($($ty: tt)*) => {
        $crate::EventSource<$crate::__private::ForLt!($($ty)*)>
    };
}

#[macro_export]
macro_rules! emit {
    ($source: expr, $event: expr) => {
        $source.with_emitter(|mut emitter| while emitter.emit_next($event).is_some() {});
    };
}

pub struct EventSource<T: ForLifetime> {
    list: Mutex<PinList<T>>,
}

impl<T: ForLifetime> EventSource<T> {
    pub const fn new() -> Self {
        Self {
            list: Mutex::new(PinList::new(unsafe { Unchecked::new() })),
        }
    }

    pub fn with_emitter(&self, emit_fn: impl FnOnce(EventEmitter<T>)) {
        let mut list = self.list.lock();

        emit_fn(EventEmitter {
            cursor: list.cursor_front_mut(),
        });
    }

    pub fn on<F>(&self, listener: F) -> EventFnFuture<F, T>
    where
        F: FnMut(T::Of<'_>) -> Option<()> + Sync,
    {
        EventFnFuture {
            source: self,
            listener,
            node: pin_list::Node::new(),
        }
    }

    pub async fn once<F, R>(&self, mut listener: F) -> R
    where
        F: FnMut(T::Of<'_>) -> Option<R> + Sync,
        R: Sync,
    {
        let mut res = None;

        self.on(|event| {
            if res.is_some() {
                return None;
            }

            listener(event).map(|output| {
                res = Some(output);
            })
        })
        .await;

        res.unwrap()
    }
}

impl<T: ForLifetime> Debug for EventSource<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventSource")
            .field("list", &self.list)
            .finish()
    }
}

#[derive(Debug)]
pub struct EventEmitter<'a, T: ForLifetime> {
    cursor: CursorMut<'a, NodeTypes<T>>,
}

impl<T: ForLifetime> EventEmitter<'_, T> {
    pub fn emit_next(&mut self, event: T::Of<'_>) -> Option<()> {
        let node = self.cursor.protected_mut()?;

        // SAFETY: Closure is pinned and the pointer is valid
        if unsafe { node.poll(event) } {
            if let Some(waker) = node.waker.take() {
                waker.wake();
            }
        }

        self.cursor.move_next();

        Some(())
    }
}

type NodeTypes<T> = dyn pin_list::Types<
    Id = pin_list::id::Unchecked,
    Protected = ListenerItem<T>,
    Unprotected = (),
    Removed = (),
>;

type PinList<T> = pin_list::PinList<NodeTypes<T>>;

type Node<T> = pin_list::Node<NodeTypes<T>>;

pin_project_lite::pin_project!(
    #[project(!Unpin)]
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct EventFnFuture<'a, F, T: ForLifetime> {
        source: &'a EventSource<T>,

        listener: F,

        #[pin]
        node: Node<T>,
    }

    impl<F, T: ForLifetime> PinnedDrop for EventFnFuture<'_, F, T> {
        fn drop(this: Pin<&mut Self>) {
            let project = this.project();
            let node = match project.node.initialized_mut() {
                Some(initialized) => initialized,
                None => return,
            };

            let _ = node.reset(&mut project.source.list.lock());
        }
    }
);

impl<'a, T: ForLifetime, F: FnMut(T::Of<'_>) -> Option<()> + Sync> Future
    for EventFnFuture<'a, F, T>
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        let mut list = this.source.list.lock();

        let node = {
            let initialized = match this.node.as_mut().initialized_mut() {
                Some(initialized) => initialized,
                None => list.push_back(this.node, ListenerItem::new(this.listener), ()),
            };

            initialized.protected_mut(&mut list).unwrap()
        };

        if node.done {
            return Poll::Ready(());
        }

        node.update_waker(cx.waker());

        Poll::Pending
    }
}

type DynClosure<'closure, T> =
    dyn for<'a> FnMut(<T as ForLifetime>::Of<'a>) -> Option<()> + Sync + 'closure;

#[derive(Debug)]
struct ListenerItem<T: ForLifetime> {
    done: bool,
    waker: Option<Waker>,
    closure_ptr: Unique<DynClosure<'static, T>>,
}

impl<T: ForLifetime> ListenerItem<T> {
    fn new<'a>(closure_ptr: &'a mut DynClosure<T>) -> Self
    where
        T: 'a,
    {
        Self {
            done: false,
            waker: None,

            // SAFETY: See ListenerItem::poll for safety requirement
            closure_ptr: unsafe {
                mem::transmute::<Unique<_>, Unique<_>>(Unique::from(closure_ptr))
            },
        }
    }

    fn update_waker(&mut self, waker: &Waker) {
        match self.waker {
            Some(ref waker) if waker.will_wake(waker) => (),

            _ => {
                self.waker = Some(waker.clone());
            }
        }
    }

    /// # Safety
    /// Calling this method is only safe if pointer of closure is valid
    unsafe fn poll(&mut self, event: T::Of<'_>) -> bool {
        if self.closure_ptr.as_mut()(event).is_some() && !self.done {
            self.done = true;
        }

        self.done
    }
}
