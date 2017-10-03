use ::rgb::{ComponentBytes, ComponentMap};
use ::std::fmt;
use ::std;

#[repr(C)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// This is it. The component type can be `u8` (aliased as `BGRA8`), `u16` (aliased as `BGRA16`), or any other type (but simple copyable types are recommended.)
///
/// You can specify a different type for alpha, but it's only for special cases (e.g. if you use a newtype like BGRA<LinearLight<u16>, u16>).
pub struct BGRA<ComponentType, AlphaComponentType=ComponentType> {
    /// Blue
    pub b:ComponentType,
    /// Green
    pub g:ComponentType,
    /// Red
    pub r:ComponentType,
    /// Alpha
    pub a:AlphaComponentType,
}


/// Alpha is last. The crate doesn't impose which value represents transparency, but usually it's 0 = transparent, 255 = opaque.
pub type BGRA8 = BGRA<u8>;

/// 16-bit BGR in machine's native endian. Alpha is last.
pub type BGRA16 = BGRA<u16>;

impl<T: Clone> BGRA<T> {
    #[must_use] #[inline(always)]
    pub fn new(r: T, g: T, b: T, a: T) -> Self {
        BGRA{r,g,b,a }
    }

    /// Iterate over all components (length=4)
    #[inline(always)]
    pub fn iter(&self) -> std::iter::Cloned<std::slice::Iter<T>> {
        self.as_slice().iter().cloned()
    }
}



impl<T: Copy, B> ComponentMap<BGRA<B>, T, B> for BGRA<T> {
    #[inline(always)]
    fn map<F>(&self, mut f: F) -> BGRA<B>
        where F: FnMut(T) -> B {
        BGRA{
            r:f(self.r),
            g:f(self.g),
            b:f(self.b),
            a:f(self.a),
        }
    }
}

impl<T> ComponentBytes<T> for BGRA<T> {
    #[inline(always)]
    fn as_slice(&self) -> &[T] {
        unsafe {
            std::slice::from_raw_parts(self as *const BGRA<T> as *const T, 4)
        }
    }

    #[inline(always)]
    fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            std::slice::from_raw_parts_mut(self as *mut BGRA<T> as *mut T, 4)
        }
    }
}

impl<T> std::iter::FromIterator<T> for BGRA<T> {
    #[inline(always)]
    fn from_iter<I: IntoIterator<Item = T>>(into_iter: I) -> Self {
        let mut iter = into_iter.into_iter();
        BGRA{r:iter.next().unwrap(), g:iter.next().unwrap(), b:iter.next().unwrap(), a:iter.next().unwrap()}
    }
}

impl<T: fmt::Display, A: fmt::Display> fmt::Display for BGRA<T,A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"bgra({},{},{},{})", self.b,self.g,self.r,self.a)
    }
}
