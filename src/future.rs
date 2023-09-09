/*
 * Created on Thu Sep 07 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

use core::{
    future::Future,
    mem,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use higher_kinded_types::ForLifetime;
use unique::Unique;

use crate::{sealed::Sealed, types::Node, EventSource};

pin_project_lite::pin_project!(
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    /// Future created with [`EventSource::on`]
    pub struct EventFnFuture<'a, F, T: ForLifetime> {
        source: &'a EventSource<T>,

        #[pin]
        listener: Sealed<F>,

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
            listener: Sealed::new(listener),
            node: pin_list::Node::new(),
        }
    }
}

impl<'a, T: ForLifetime, F: FnMut(T::Of<'_>, &mut ControlFlow) + Send + Sync> Future
    for EventFnFuture<'a, F, T>
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        let mut list = this.source.list.lock();
        let node = {
            let initialized = match this.node.as_mut().initialized_mut() {
                Some(initialized) => initialized,
                None => list.push_back(
                    this.node,
                    ListenerItem::new(
                        Unique::new(this.listener.get_ptr_mut().as_ptr() as _).unwrap(),
                    ),
                    (),
                ),
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
    dyn for<'a, 'b> FnMut(<T as ForLifetime>::Of<'a>, &'b mut ControlFlow) + Send + Sync + 'closure;

#[derive(Debug)]
pub struct ListenerItem<T: ForLifetime> {
    done: bool,
    waker: Option<Waker>,
    closure_ptr: Unique<DynClosure<'static, T>>,
}

impl<T: ForLifetime> ListenerItem<T> {
    fn new(closure: Unique<DynClosure<T>>) -> Self {
        Self {
            done: false,
            waker: None,

            // SAFETY: Extend lifetime and manage manually, see ListenerItem::poll for safety requirement
            closure_ptr: unsafe { mem::transmute::<Unique<_>, Unique<_>>(closure) },
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
    /// Calling this method is only safe if pointer to closure is valid
    pub unsafe fn poll(&mut self, event: T::Of<'_>) -> bool {
        let mut flow = ControlFlow {
            done: self.done,
            propagation: true,
        };

        self.closure_ptr.as_mut()(event, &mut flow);

        if flow.done && !self.done {
            self.done = true;

            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }

        flow.propagation
    }
}

#[derive(Debug)]
/// Control current listener's behaviour
pub struct ControlFlow {
    done: bool,
    propagation: bool,
}

impl ControlFlow {
    /// Stop propagation of the current event
    pub fn stop_propagation(&mut self) {
        if self.propagation {
            self.propagation = false;
        }
    }

    /// Check if listener is finished already
    pub const fn done(&self) -> bool {
        self.done
    }

    /// Mark listener as finished
    pub fn set_done(&mut self) {
        if !self.done {
            self.done = true;
        }
    }
}
