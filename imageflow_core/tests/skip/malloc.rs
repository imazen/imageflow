
// Test absurdly high malloc (address space exhaustion)
// Test malloc higher than swap, reduce until success
// Write zeroes to huge malloc, verify process doesn't crash (hey, overcommit isn't what you think)
// Free
// Malloc 1mb chunks until they fail. See if smaller mallocs succeed, and how many of them. Wait a bit, then try an extra 1mb chunk
// Have time limit


// #[test]
// fn leak_mem() {
//
//    let mut v = Vec::with_capacity(333);
//    v.push(0u8);
//    std::mem::forget(v)
// }




// #[test]
// fn test_panics(){
//    let result = ::std::panic::catch_unwind(|| {
//        panic!("oh no!");
//    });
//
//    if let Err(err) = result {
//        let str = format!("{:?}", err.downcast::<&'static str>());
//        assert_eq!(str, "");
//    }
// }

#[test]
fn test_panics2() {
    // let input_bytes = [0u8;3000000];
    //    let result = ::std::panic::catch_unwind(|| {
    //        let input_bytes = [2u8;10 * 1024 * 1024 * 1024];
    //    });

    //    if let Err(err) = result {
    //        let str = format!("{:?}", err.downcast::<&'static str>());
    //        assert_eq!(str, "");
    //    }
}


// fn new_oom_handler() -> ! {
//    panic!("OOM");
// }
//
// #[allow(unused_variables)]
// #[test]
// fn test_panics3(){
//
//    alloc::oom::set_oom_handler(new_oom_handler);
//
//    // let input_bytes = [0u8;3000000];
//    let result = ::std::panic::catch_unwind(|| {
//        let b = vec![0;30 * 1024 * 1024 * 1024];
//    });
//
//    if let Err(err) = result {
//        let str = format!("{:?}", err.downcast::<&'static str>());
//        assert_eq!(str, "Ok(\"OOM\")");
//    }
// }
