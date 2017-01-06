use ::imageflow_helpers as hlp;
use ::imageflow_helpers::preludes::from_std::*;
use ::imageflow_core::clients::stateless;
use ::imageflow_core::clients::fluent;
use ::s;
use ::rustc_serialize::base64::FromBase64;


const BLUE_PNG32_200X200_B64:&'static str = "iVBORw0KGgoAAAANSUhEUgAAAMgAAADICAYAAACtWK6eAAABiUlEQVR42u3TgRAAQAgAsA/qkaKLK48EIug2h8XP6gesQhAQBAQBQUAQEAQEAUFAEBAEEAQEAUFAEBAEBAFBQBAQBAQRBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQBBQBAQBAQBQUAQEAQEAUEAQUAQEAQEAUFAEBAEBAFBQBBAEBAEBAFBQBAQBAQBQUAQQBAQBAQBQUAQEAQEAUFAEBAEEAQEAUFAEBAEBAFBQBAQBAQRBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQQBQUAQEAQEAUFAEBAEBAFBAEFAEBAEBAFBQBAQBAQBQUAQQBAQBAQBQUAQEAQEAUFAEEAQEAQEAUFAEBAEBAFBQBAQBBAEBAFBQBAQBAQBQUAQEAQQBAQBQUAQEAQEAUFAEBAEBAEEAUFAEBAEBAFBQBAQBAQBQQQBQUAQEAQEAUFAEBAEBAFBAEFAEBAEBAFBQBAQBAQBQUAQQUAQEAQEAUFAEBAEBIGLBkZ+sahOjkyUAAAAAElFTkSuQmCC";

fn smoke_png_to_png() {
    let framewise = fluent::fluently()
        .decode(0)
        .constrain_within(Some(40), Some(40), Some(s::ConstraintResamplingHints::with(None, Some(25f32))))
        .encode(1, s::EncoderPreset::libpng32()).builder().to_framewise();

    let bytes = BLUE_PNG32_200X200_B64.from_base64().unwrap();

    let req = stateless::BuildRequest{
        export_graphs_to: None,
        inputs: vec![stateless::BuildInput{bytes: &bytes, io_id: 0}],
        framewise: framewise
    };
    let _ = stateless::LibClient{}.build(req).unwrap();
}

pub fn smoke_test_core() {
    smoke_png_to_png();
}