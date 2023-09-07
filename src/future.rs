/*
 * Created on Thu Sep 07 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

use core::{
    future::Future,
    mem,
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll, Waker},
};

use higher_kinded_types::ForLifetime;

use crate::{types::Node, EventSource};

pin_project_lite::pin_project!(
    #[project(!Unpin)]
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct EventFnFuture<'a, F, T: ForLifetime> {
        source: &'a EventSource<T>,

        #[pin]
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

impl<'a, T: ForLifetime, F> EventFnFuture<'a, F, T> {
    pub(super) const fn new(source: &'a EventSource<T>, listener: F) -> Self {
        Self {
            source,
            listener,
            node: pin_list::Node::new(),
        }
    }
}

// SAFETY: Everything in EventFnFuture is safe to send and closure is Send
unsafe impl<F: Send, T: ForLifetime> Send for EventFnFuture<'_, F, T> {}

// SAFETY: Everything in EventFnFuture is safe to sync and closure is Sync
unsafe impl<F: Sync, T: ForLifetime> Sync for EventFnFuture<'_, F, T> {}

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
    dyn for<'a> FnMut(<T as ForLifetime>::Of<'a>) -> Option<()> + 'closure;

#[derive(Debug)]
pub(crate) struct ListenerItem<T: ForLifetime> {
    done: bool,
    waker: Option<Waker>,
    closure_ptr: NonNull<DynClosure<'static, T>>,
}

impl<T: ForLifetime> ListenerItem<T> {
    fn new<'a>(closure_ptr: Pin<&'a mut DynClosure<T>>) -> Self
    where
        T: 'a,
    {
        Self {
            done: false,
            waker: None,

            // SAFETY: See ListenerItem::poll for safety requirement
            closure_ptr: unsafe {
                mem::transmute::<NonNull<_>, NonNull<_>>(NonNull::from(&*closure_ptr))
            },
        }
    }

    pub fn update_waker(&mut self, waker: &Waker) {
        match self.waker {
            Some(ref waker) if waker.will_wake(waker) => (),

            _ => {
                self.waker = Some(waker.clone());
            }
        }
    }

    pub fn take_waker(&mut self) -> Option<Waker> {
        self.waker.take()
    }

    /// # Safety
    /// Calling this method is only safe if pointer of closure is valid
    pub unsafe fn poll(&mut self, event: T::Of<'_>) -> bool {
        if self.closure_ptr.as_mut()(event).is_some() && !self.done {
            self.done = true;
        }

        self.done
    }
}
