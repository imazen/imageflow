// Taken from https://github.com/pornel/image-gif-dispose (MIT/Apache dual license)

use std::iter;

pub trait Subimage<T: Iterator> {
    fn subimage(self, left: usize, top: usize, sub_width: usize, sub_height: usize, stride: usize) -> Iter<T>;
}

impl<T: Iterator> Subimage<T> for T {
    fn subimage(self, left: usize, top: usize, sub_width: usize, sub_height: usize, stride: usize) -> Iter<T> {
        assert!(sub_width > 0);
        assert!(sub_width <= stride);
        assert!(left.checked_add(sub_width).unwrap() <= stride);
        assert!(sub_height > 0);

        Iter {
            i: sub_width,
            width: sub_width,
            gap: stride - sub_width,
            inner: self.skip(top.checked_mul(stride).unwrap().checked_add(left).unwrap())
                .take((sub_height - 1).checked_mul(stride).unwrap().checked_add(sub_width).unwrap()),
        }
    }
}

pub struct Iter<T: Iterator> {
    i: usize,
    width: usize,
    gap: usize,
    inner: iter::Take<iter::Skip<T>>,
}

impl<T: Iterator> Iterator for Iter<T> {
    type Item = T::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if self.i == 0 {
            for _ in 0..self.gap {
                self.inner.next();
            }
            self.i = self.width;
        }
        self.i -= 1;
        self.inner.next()
    }
}

#[test]
fn test_iter() {
    let d = [1u8,2,3,4,
        5,6,7,8,
        255,255,255,255];
    let i:Vec<_> = d.iter().cloned().subimage(0,0,2,2, 4).collect();
    assert_eq!(i, vec![1,2, 5,6]);

    let i:Vec<_> = d.iter().cloned().subimage(1,0,2,2, 4).collect();
    assert_eq!(i, vec![2,3, 6,7]);

    let i:Vec<_> = d.iter().cloned().subimage(2,1,2,1, 4).collect();
    assert_eq!(i, vec![7,8]);
}
