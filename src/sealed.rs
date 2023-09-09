/*
 * Created on Fri Sep 08 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

use core::{pin::Pin, ptr::NonNull};

use sync_wrapper::SyncWrapper;

pin_project_lite::pin_project! {
    #[project(!Unpin)]
    #[derive(Debug)]
    pub struct Sealed<T> {
        inner: SyncWrapper<T>,
    }
}

impl<T> Sealed<T> {
    pub const fn new(inner: T) -> Self {
        Self {
            inner: SyncWrapper::new(inner),
        }
    }

    pub fn get_ptr_mut(self: Pin<&mut Self>) -> NonNull<T> {
        NonNull::from(self.project().inner.get_mut())
    }
}
