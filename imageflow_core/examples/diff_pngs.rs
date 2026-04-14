fn main() {
    let a_path = std::env::args().nth(1).unwrap();
    let b_path = std::env::args().nth(2).unwrap();
    let ai = lodepng::decode32(&std::fs::read(&a_path).unwrap()).unwrap();
    let bi = lodepng::decode32(&std::fs::read(&b_path).unwrap()).unwrap();
    assert_eq!((ai.width, ai.height), (bi.width, bi.height));
    let mut diff = 0usize;
    let mut hist = [0u32; 256];
    let (mut dr, mut dg, mut db, mut da) = (0i32, 0i32, 0i32, 0i32);
    for (p, q) in ai.buffer.iter().zip(bi.buffer.iter()) {
        let r = (p.r as i32 - q.r as i32).abs();
        let g = (p.g as i32 - q.g as i32).abs();
        let b = (p.b as i32 - q.b as i32).abs();
        let a_ = (p.a as i32 - q.a as i32).abs();
        let m = r.max(g).max(b);
        if m != 0 || a_ != 0 {
            diff += 1;
        }
        hist[m as usize] += 1;
        dr = dr.max(r);
        dg = dg.max(g);
        db = db.max(b);
        da = da.max(a_);
    }
    let total = ai.buffer.len();
    println!("{}x{} = {} pixels", ai.width, ai.height, total);
    println!("differing: {} ({:.2}%)", diff, 100.0 * diff as f64 / total as f64);
    println!("max deltas: R={} G={} B={} A={}", dr, dg, db, da);
    println!("max-channel-delta histogram (only showing non-zero bins):");
    for (d, c) in hist.iter().enumerate() {
        if *c > 0 {
            println!("  delta={:3}: {} pixels ({:.2}%)", d, c, 100.0 * *c as f64 / total as f64);
        }
    }
}
