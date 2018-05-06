use std::fs::{File};
use std::io::{Write, BufWriter};
use std::path::{Path};
extern crate imageflow_core as fc;
extern crate imageflow_types as s;
use fc::clients::stateless;

fn create_blank(dir: &str, filename_without_ext: &str, w: usize, h: usize, encoder: s::EncoderPreset){

    //Invalid read here; the result of create_canvas is not being accessed correctly.
    let req = stateless::BuildRequest{
        inputs: vec![],
        framewise: s::Framewise::Steps(
            vec![
            s::Node::CreateCanvas{ w: w, h: h, format: s::PixelFormat::Bgr24, color: s::Color::Black},
            s::Node::Encode{ io_id: 0, preset: encoder }
            ]
        ),
        export_graphs_to: None //Some(std::path::PathBuf::from(format!("./{}/{}_debug", dir, filename_without_ext)))

    };
    let result = stateless::LibClient::new().build(req).unwrap();
    let ref out: stateless::BuildOutput = result.outputs[0];
    let mut path = Path::new(dir).join(filename_without_ext);
    path.set_extension(&out.file_ext);

    let mut file = BufWriter::new(File::create(&path).unwrap());
    file.write(&out.bytes).unwrap();
}
fn setup(dir: &str){

    create_blank(dir, "200x200", 200, 200, s::EncoderPreset::libjpeg_turbo_classic());
    //create_blank(dir, "200x200", 200, 200, s::EncoderPreset::libpng32());
//    let to_path =  Path::new(dir).join("valgrind_suppressions.txt");
//    std::fs::copy("../../valgrind_suppressions.txt", to_path).unwrap();
}

#[test]
fn repro_mem_access_err(){
    setup(".");
}
