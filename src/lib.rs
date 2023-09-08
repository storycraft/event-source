/*
 * Created on Thu Aug 10 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

#![no_std]
#![doc = include_str!("../README.md")]

#[doc(hidden)]
pub mod __private;
pub mod future;
mod types;
mod sealed;

use core::fmt::{self, Debug};

use future::EventFnFuture;
use higher_kinded_types::ForLifetime;
use parking_lot::Mutex;

use pin_list::{id::Unchecked, CursorMut};

use types::{NodeTypes, PinList};

#[macro_export]
/// Higher kinded type helper for [`struct@EventSource`]
macro_rules! EventSource {
    ($($ty: tt)*) => {
        $crate::EventSource<$crate::__private::ForLt!($($ty)*)>
    };
}

#[macro_export]
/// Emit event. As methods can't do mutable reborrowing correctly, you should use this macro.
macro_rules! emit {
    ($source: expr, $event: expr) => {
        $source.with_emitter(|mut emitter| while emitter.emit_next($event).is_some() {});
    };
}

/// Event source
pub struct EventSource<T: ForLifetime> {
    list: Mutex<PinList<T>>,
}

// SAFETY: EventSource doesn't own any data
unsafe impl<T: ForLifetime> Send for EventSource<T> {}

// SAFETY: Sync guaranteed by Mutex
unsafe impl<T: ForLifetime> Sync for EventSource<T> {}

impl<T: ForLifetime> Debug for EventSource<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventSource")
            .field("list", &self.list)
            .finish()
    }
}

impl<T: ForLifetime> EventSource<T> {
    /// Create new [`struct@EventSource`]
    pub const fn new() -> Self {
        Self {
            // SAFETY: There is only one variant of [`Pinlist`]
            list: Mutex::new(PinList::new(unsafe { Unchecked::new() })),
        }
    }

    /// Create [`EventEmitter`] for this [`struct@EventSource`]
    pub fn with_emitter(&self, emit_fn: impl FnOnce(EventEmitter<T>)) {
        let mut list = self.list.lock();

        emit_fn(EventEmitter {
            cursor: list.cursor_front_mut(),
        });
    }

    /// Listen event until listener returns [`Option::Some`]
    ///
    /// It can be called after woken if another event occurred before task continue.
    pub fn on<F>(&self, listener: F) -> EventFnFuture<F, T>
    where
        F: FnMut(T::Of<'_>) -> Option<()> + Sync,
    {
        EventFnFuture::new(self, listener)
    }

    /// Listen event until listener returns [`Option::Some`]
    ///
    /// Unlike [`EventSource::on`] it will ignore every events once listener returns with [`Option::Some`].
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

/// Struct for emitting different value for each listeners
#[derive(Debug)]
pub struct EventEmitter<'a, T: ForLifetime> {
    cursor: CursorMut<'a, NodeTypes<T>>,
}

impl<T: ForLifetime> EventEmitter<'_, T> {
    /// Emit event to next listener
    pub fn emit_next(&mut self, event: T::Of<'_>) -> Option<()> {
        let node = self.cursor.protected_mut()?;

        // SAFETY: Closure is pinned and the pointer is valid
        if unsafe { node.poll(event) } {
            if let Some(waker) = node.take_waker() {
                waker.wake();
            }
        }

        self.cursor.move_next();

        Some(())
    }
}
