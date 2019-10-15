use std::ops::Deref;

use evmap::ShallowCopy;

use crate::{Counter, Gauge, Histogram};

pub enum DynCow<'a, T: ?Sized> {
    Borrowed(&'a T),
    Owned(Box<T>),
}

impl<'a, T: ?Sized> DynCow<'a, T> {
    pub unsafe fn from_ptr(ptr: *const T) -> Self {
        Self::Borrowed(&*ptr)
    }

    pub fn as_ptr(&self) -> *const T {
        &**self as *const T
    }
}

impl<'a, T: ?Sized> Deref for DynCow<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Self::Borrowed(x) => x,
            Self::Owned(x) => &*x,
        }
    }
}

impl<'a, T: ?Sized> From<&'a T> for DynCow<'a, T> {
    fn from(val: &'a T) -> Self {
        Self::Borrowed(val)
    }
}

impl<'a, H> From<Box<H>> for DynCow<'a, dyn Histogram + 'a>
where
    H: Histogram + 'a,
{
    fn from(val: Box<H>) -> Self {
        Self::Owned(val)
    }
}

impl<'a, C> From<Box<C>> for DynCow<'a, dyn Counter + 'a>
where
    C: Counter + 'a,
{
    fn from(val: Box<C>) -> Self {
        Self::Owned(val)
    }
}

impl<'a, G> From<Box<G>> for DynCow<'a, dyn Gauge + 'a>
where
    G: Gauge + 'a,
{
    fn from(val: Box<G>) -> Self {
        Self::Owned(val)
    }
}

impl<'a, H> From<&'a H> for DynCow<'a, dyn Histogram + 'a>
where
    H: Histogram + 'a,
{
    fn from(val: &'a H) -> Self {
        Self::Borrowed(val)
    }
}

impl<'a, C> From<&'a C> for DynCow<'a, dyn Counter + 'a>
where
    C: Counter + 'a,
{
    fn from(val: &'a C) -> Self {
        Self::Borrowed(val)
    }
}

impl<'a, G> From<&'a G> for DynCow<'a, dyn Gauge + 'a>
where
    G: Gauge + 'a,
{
    fn from(val: &'a G) -> Self {
        Self::Borrowed(val)
    }
}

impl<'a, T: ?Sized> PartialEq for DynCow<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr()
    }
}

impl<'a, T: ?Sized> Eq for DynCow<'a, T> {}

impl<'a, T: ?Sized> ShallowCopy for DynCow<'a, T> {
    unsafe fn shallow_copy(&mut self) -> Self {
        match self {
            Self::Borrowed(x) => Self::Borrowed(x),
            Self::Owned(b) => Self::Owned(b.shallow_copy()),
        }
    }
}
