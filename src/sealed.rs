/*
 * Created on Fri Sep 08 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

#[derive(Debug)]
pub struct Sealed<T: ?Sized>(T);

impl<T> Sealed<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T: ?Sized> Sealed<T> {
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

// SAFETY: Getting inner value without using pointer is impossible
unsafe impl<T: ?Sized> Sync for Sealed<T> {}
