/*
 * Created on Thu Aug 10 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

#![no_std]
#![doc = include_str!("../README.md")]

#[doc(hidden)]
pub mod __private;
mod future;
mod sealed;
mod types;

pub use future::{ControlFlow, EventFnFuture};

use core::fmt::{self, Debug};

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

    /// Listen events
    ///
    /// It can be called after woken if another event occurred before task continue.
    /// 
    /// Closure must be [`Sync`] to be called safely
    pub fn on<F>(&self, listener: F) -> EventFnFuture<F, T>
    where
        F: FnMut(T::Of<'_>, &mut ControlFlow) + Sync,
    {
        EventFnFuture::new(self, listener)
    }

    /// Listen event until listener returns [`Option::Some`]
    ///
    /// Unlike [`EventSource::on`] it will ignore every events once listener is done or returns with [`Option::Some`].
    pub async fn once<F, R>(&self, mut listener: F) -> Option<R>
    where
        F: FnMut(T::Of<'_>, &mut ControlFlow) -> Option<R> + Sync,
        R: Sync,
    {
        let mut out = None;

        self.on(|event, flow| {
            if flow.done() {
                return;
            }

            if let output @ Some(_) = listener(event, flow) {
                out = output;
                flow.set_done();
            }
        })
        .await;

        out
    }
}

/// Struct for emitting values for each listeners
#[derive(Debug)]
pub struct EventEmitter<'a, T: ForLifetime> {
    cursor: CursorMut<'a, NodeTypes<T>>,
}

impl<T: ForLifetime> EventEmitter<'_, T> {
    /// Emit event to next listener
    pub fn emit_next(&mut self, event: T::Of<'_>) -> Option<()> {
        let node = self.cursor.protected_mut()?;

        // SAFETY: Every listener closure is Sync and the pointer is valid
        if unsafe { !node.poll(event) } {
            return None;
        }

        self.cursor.move_next();

        Some(())
    }
}
