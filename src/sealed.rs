/*
 * Created on Fri Sep 08 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

use core::ptr::NonNull;

#[derive(Debug)]
pub struct Sealed<T: ?Sized>(T);

impl<T> Sealed<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T: ?Sized> Sealed<T> {
    pub fn get_ptr_mut(&mut self) -> NonNull<T> {
        NonNull::from(&mut self.0)
    }
}
