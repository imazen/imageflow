// Usage: dump in.raw w h ow oh opaque|alpha out.raw   (BGRA8 raw, resize in linear light with Robidoux)
use zenresize::{AlphaMode, Filter, PixelDescriptor, ResizeConfig, StreamingResize};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (w, h, ow, oh): (u32, u32, u32, u32) = (a[2].parse().unwrap(), a[3].parse().unwrap(), a[4].parse().unwrap(), a[5].parse().unwrap());
    let input = std::fs::read(&a[1]).unwrap();
    let desc = if a[6] == "opaque" { PixelDescriptor::BGRA8_SRGB.with_alpha(None) } else { PixelDescriptor::BGRA8_SRGB.with_alpha(Some(AlphaMode::Straight)) };
    let cfg = ResizeConfig::builder(w, h, ow, oh).filter(Filter::Robidoux).format(desc).linear().build();
    let mut s = StreamingResize::new(&cfg);
    eprintln!("{:?}", s.working_format());
    let mut out = Vec::with_capacity((ow * oh * 4) as usize);
    let stride = w as usize * 4;
    for y in 0..h as usize {
        s.push_row(&input[y * stride..(y + 1) * stride]).unwrap();
        while let Some(r) = s.next_output_row() { out.extend_from_slice(r); }
    }
    let n = s.finish();
    for _ in 0..n { out.extend_from_slice(s.next_output_row().unwrap()); }
    std::fs::write(&a[7], &out).unwrap();
}
