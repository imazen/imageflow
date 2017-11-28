use std::cell::*;

/// Provides interior mutability for a add-only set.
/// Items can be added (a reference is returned)
/// References can be looked up via a pointer to the item
#[derive(Debug,Clone, PartialEq)]
pub struct AppendOnlySet<T>{
    slots: RefCell<Vec<Box<T>>>
}
impl<T> AppendOnlySet<T> {
    pub fn with_capacity(slots: usize) -> AppendOnlySet<T> {
        AppendOnlySet {
            slots: RefCell::new(Vec::with_capacity(slots))
        }
    }
    // &T's lifetime can't exceed that of AppendOnlySet's
    pub fn add(&self, value: T) -> &T {
        //Boxing T means that the address will never change,
        //Even if the vector owning the Boxed values is resized and moved via push
        let boxed = Box::new(value);
        //We take the stable address of boxed T
        let ptr = &*boxed as *const T;
        self.slots.borrow_mut().push(boxed);
        //Change lifetime to that of of AppendOnlySet instead of slots.borrow_mut()
        unsafe { &*ptr }
    }

    // We return a reference to the cell
    // First match wins; multiple matches would indicate that two Boxes had been
    // allocated with the same address.
    // Pointer is never dereferenced
    pub fn get_reference(&self, ptr: *const T) -> Option<&T> {
        for item in self.slots.borrow().iter() {
            let item_ptr = &**item as *const T;
            if ptr == item_ptr {
                //Change lifetime to that of of AppendOnlySet instead of slots.borrow()
                return Some(unsafe { &*item_ptr })
            }
        }
        None
    }
    // Pointer is never dereferenced
    pub fn contains(&self, item: *const T) -> bool {
        self.get_reference(item).is_some()
    }

    ///
    /// If you have a mutable reference, you can do whatever you like, since
    /// there cannot be any other outstanding references
    pub fn get_mut_vec(&mut self) -> &mut Vec<Box<T>>{
        self.slots.get_mut()
    }

    pub fn iter(&self) -> IterAppendOnlySet<T>{
        IterAppendOnlySet{
            set: self,
            index: 0
        }
    }

}



pub struct IterAppendOnlySet<'a, T: 'a>{
    set: &'a AppendOnlySet<T>,
    index: usize
}

impl<'a, T> Iterator for IterAppendOnlySet<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let vec_ref = self.set.slots.borrow();
        let result = vec_ref.get(self.index).map(|b| {
            //Change lifetime from 'vec_ref to 'a
            unsafe{ &* (&**b as *const T)}
        });
        self.index += 1;
        result
    }
}

/// Provides a simple set which has interior mutability.
/// Removal is offered, but can fail at runtime if there is an active borrow.
/// Removal leaves holes, and there is no way to reclaim that space.
///
///
#[derive(Debug,Clone, PartialEq)]
pub struct AddRemoveSet<T>{
    inner: AppendOnlySet<RefCell<Option<T>>>
}
impl<T> AddRemoveSet<T> {
    pub fn with_capacity(slots: usize) -> AddRemoveSet<T> {
        AddRemoveSet {
            inner: AppendOnlySet::with_capacity(slots)
        }
    }
    pub fn add(&self, value: T) -> Ref<T> {
        Ref::map(self.inner.add(RefCell::new(Some(value))).borrow(), |t| t.as_ref().unwrap())
    }
    pub fn add_mut(&self, value: T) -> RefMut<T> {
        RefMut::map(self.inner.add(RefCell::new(Some(value))).borrow_mut(), |t| t.as_mut().unwrap())
    }

    pub fn iter(&self) -> IterAddRemoveSet<T>{
        IterAddRemoveSet{ inner: self.inner.iter() }
    }

    /// Ok(None) means it definitely doesn't exist in the set
    /// Err() means we couldn't access all the items in
    //    pub fn get_reference(&self, ptr: *const T) -> Result<Option<&T>,BorrowMutError>{
    //        self.iter().find(|r| if let Ok(reference) = r { reference as *const T == ptr } else {false}).map(|v| Some(v)).unwrap_or(Ok(None))
    //    }

    pub fn iter_mut(&self) -> IterMutAddRemoveSet<T>{
        IterMutAddRemoveSet{ inner: self.inner.iter() }
    }


    pub fn mut_clear(&mut self){
        self.inner.get_mut_vec().clear();
    }

    pub fn clear(&self) -> Result<(),BorrowMutError>{
        for refcell in self.inner.iter() {
            let mut ref_obj = refcell.try_borrow_mut()?;
            *ref_obj = None;
        }
        Ok(())
    }

    /// Ok(true) - removed. Ok(false) - certainly didn't exist. Err() - either borrowed or didn't exist (unknowable)
    pub fn try_remove(&self, v: *const T) -> Result<bool, BorrowMutError>{
        match self.try_get_option_reference_mut(v)? {
            Some(mut ref_obj) => {
                *ref_obj = None;
                Ok(true)
            },
            None => Ok(false),
        }
    }

    pub fn try_contains(&self, v: *const T) -> Result<bool, BorrowError>{
        self.try_get_reference(v).map(|opt| opt.is_some())
    }

    pub fn try_get_reference(&self, v: *const T) -> Result<Option<Ref<T>>, BorrowError>{
        let mut last_error = None;
        for refcell in self.inner.iter() {
            match refcell.try_borrow() {
                Ok(ref_obj) => {
                    let other_ptr = ref_obj.as_ref().map(|v| v as *const T);
                    if Some(v) == other_ptr{
                        return Ok(Some(Ref::map(ref_obj, |r| r.as_ref().unwrap())));
                    }
                }
                Err(e) => { last_error = Some(e); }
            }
        }
        if let Some(last_error) = last_error {
            Err(last_error)
        }else{
            Ok(None)
        }
    }
    pub fn try_get_reference_mut(&self, v: *const T) -> Result<Option<RefMut<T>>, BorrowMutError>{
        self.try_get_option_reference_mut(v).map(|opt| opt.and_then(|ref_obj| {
            if ref_obj.is_some() { Some(RefMut::map(ref_obj, |r| r.as_mut().unwrap())) } else {None}
        }))
    }
    fn try_get_option_reference_mut(&self, v: *const T) -> Result<Option<RefMut<Option<T>>>, BorrowMutError>{
        let mut last_error = None;
        for refcell in self.inner.iter() {
            match refcell.try_borrow_mut() {
                Ok(ref_obj) => {
                    if ref_obj.is_some() && ref_obj.as_ref().unwrap() as *const T == v {
                        return Ok(Some(ref_obj));
                    }
                }
                Err(e) => { last_error = Some(e); }
            }
        }
        if let Some(last_error) = last_error {
            Err(last_error)
        }else{
            Ok(None)
        }
    }
}

pub struct IterAddRemoveSet<'a, T: 'a>{
    inner: IterAppendOnlySet<'a, RefCell<Option<T>>>
}

impl<'a, T> Iterator for IterAddRemoveSet<'a, T> {
    type Item = Result<Ref<'a,T>, BorrowError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next(){
            None => None,
            Some(cell) => {
                match cell.try_borrow(){
                    Ok(ref_obj) => {
                        if ref_obj.is_none(){
                            None
                        }else{
                            Some(Ok(Ref::map(ref_obj, |r| r.as_ref().unwrap())))
                        }
                    }
                    Err(e) => Some(Err(e))
                }
            }
        }
    }
}

pub struct IterMutAddRemoveSet<'a, T: 'a>{
    inner: IterAppendOnlySet<'a, RefCell<Option<T>>>
}

impl<'a, T> Iterator for IterMutAddRemoveSet<'a, T> {
    type Item = Result<RefMut<'a,T>, BorrowMutError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next(){
            None => None,
            Some(cell) => {
                match cell.try_borrow_mut(){
                    Ok(ref_obj) => {
                        if ref_obj.is_none(){
                            None
                        }else{
                            Some(Ok(RefMut::map(ref_obj, |r| r.as_mut().unwrap())))
                        }
                    }
                    Err(e) => Some(Err(e))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests{
    use ::std::cell::*;
    use super::*;


    #[derive(Clone, PartialEq,Debug)]
    struct Container{
        objects: AppendOnlySet<RefCell<Option<Child>>>,
        b: AddRemoveSet<Child>
    }
    impl Container{
        pub fn new() -> Container{
            Container{
                objects: AppendOnlySet::with_capacity(4),
                b: AddRemoveSet::with_capacity(1)
            }
        }

        pub fn add_child_get_cell(&self) -> &RefCell<Option<Child>>{
            self.objects.add(RefCell::new(Some(Child{})))
        }

        pub fn add_child_get_ref(&self) -> RefMut<Option<Child>>{
            self.objects.add(RefCell::new(Some(Child{}))).borrow_mut()
        }

    }


    #[derive(Clone, PartialEq,Debug)]
    struct Child{

    }
    impl Child{
        pub fn do_a_thing(&mut self, _: &Container){

        }
    }

    #[test]
    fn test_sets_with_interior_mutability(){
        let g = Container::new();

        let child = g.add_child_get_cell();
        child.borrow_mut().as_mut().unwrap().do_a_thing(&g);
        assert_eq!(g.objects.get_reference(child as *const RefCell<Option<Child>>), Some(child));
        assert!(g.objects.contains(&*child));
        let mut c2 = g.add_child_get_ref();
        assert!(g.objects.contains(&*child));
        c2.as_mut().unwrap().do_a_thing(&g);

        let c3_ptr;
        {
            let c3 = g.b.add(Child {});
            c3_ptr = &*c3 as *const Child;
            g.b.add_mut(Child {}).do_a_thing(&g);
            for _ in 0..30 {
                let addl_ptr;
                {
                    let mut addl = g.b.add_mut(Child {});
                    addl.do_a_thing(&g);
                    addl_ptr = &*addl as *const Child;
                }
                assert!(g.b.try_contains(&*c3).unwrap());
                assert!(g.b.try_remove(addl_ptr).unwrap());
                assert!(!g.b.try_contains(addl_ptr).unwrap());
            }
        }
        assert!(g.b.try_remove(c3_ptr).unwrap());
        assert!(!g.b.try_contains(c3_ptr).unwrap());
    }



//    mod experiment_with_parent_references{
//        use super::super::*;
//        use ::std::cell::RefMut;
//        struct P<'a>{ c: AddRemoveSet<C<'a>> } struct C<'a>{ p: &'a P<'a> }
//        impl<'a> P<'a>{
//
//            pub fn new() -> P<'static>{
//                P{
//                    c: AddRemoveSet::with_capacity(1)
//                }
//            }
//
//            pub fn add_child(&'a self) -> RefMut<C<'a>>{
//                self.c.add_mut(C{p:self})
//            }
//        }
//
//        #[test]
//        fn tryit(){
//
//            let mut a = P::new();
//
//            let mut child = a.add_child();
//
//        }
//
//
//
//    }
}
