/*
 * Created on Thu Sep 07 2023
 *
 * Copyright (c) storycraft. Licensed under the MIT Licence.
 */

use crate::future::ListenerItem;

pub(crate) type NodeTypes<T> = dyn pin_list::Types<
    Id = pin_list::id::Unchecked,
    Protected = ListenerItem<T>,
    Unprotected = (),
    Removed = (),
>;

pub(crate) type PinList<T> = pin_list::PinList<NodeTypes<T>>;

pub(crate) type Node<T> = pin_list::Node<NodeTypes<T>>;
