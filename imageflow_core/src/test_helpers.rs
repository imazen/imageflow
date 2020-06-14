use crate::internal_prelude::works_everywhere::*;
use crate::clients::stateless;


pub mod process_testing {
    use crate::internal_prelude::works_everywhere::*;
    use ::imageflow_helpers::process_testing::*;
    use super::*;
    use crate::clients::stateless;


    pub trait ProcTestContextExtras {
        fn write_json<T, P: AsRef<Path>>(&self, filename: P, info: &T)
            where T: serde::Serialize;

        fn create_blank_image_here<P: AsRef<Path>>(&self,
                                                   filename_without_ext: P,
                                                   w: u32,
                                                   h: u32,
                                                   encoder: s::EncoderPreset);
    }

    pub trait ProcOutputExtras {
        fn parse_stdout_as<'de, T>(&'de self) -> std::result::Result<T, serde_json::error::Error>
            where T: serde::Deserialize<'de>;
    }

    impl ProcOutputExtras for ProcOutput {
        fn parse_stdout_as<'de, T>(&'de self) -> std::result::Result<T, serde_json::error::Error>
            where T: serde::Deserialize<'de>
        {
            serde_json::from_slice(self.stdout_bytes())
        }
    }

    impl ProcTestContextExtras for ProcTestContext {
        fn write_json<T, P: AsRef<Path>>(&self, filename: P, info: &T)
            where T: serde::Serialize
        {
            let bytes = ::serde_json::to_vec_pretty(info).unwrap();
            self.write_file(filename, &bytes);
        }

        fn create_blank_image_here<P: AsRef<Path>>(&self,
                                                   filename_without_ext: P,
                                                   w: u32,
                                                   h: u32,
                                                   encoder: s::EncoderPreset) {
            let out = BlankImage {
                w: w,
                h: h,
                encoding: encoder,
                color: s::Color::Black
            }.generate();


            let mut path = self.working_dir().join(filename_without_ext);
            path.set_extension(&out.file_ext);

            self.write_file(path.file_name().unwrap().to_str().unwrap(), &out.bytes);
        }
    }
}

#[derive(Clone,Debug,PartialEq)]
pub struct BlankImage{
    pub w: u32,
    pub h: u32,
    pub color: s::Color,
    pub encoding: s::EncoderPreset
}

impl BlankImage{
    pub fn generate(&self) -> stateless::BuildOutput{
        // Invalid read here; the result of create_canvas is not being accessed correctly.
        let req = stateless::BuildRequest {
            inputs: vec![],
            framewise: s::Framewise::Steps(vec![s::Node::CreateCanvas {
                w: self.w as usize,
                h: self.h as usize,
                format: s::PixelFormat::Bgr32,
                color: self.color.clone(),
            },
            s::Node::Encode {
                io_id: 0,
                preset: self.encoding.clone(),
            }]),
            export_graphs_to: None, /* Some(std::path::PathBuf::from(format!("./{}/{}_debug", dir, filename_without_ext))) */
        };

        let result = crate::clients::stateless::LibClient::new().build(req).unwrap();
        result.outputs.into_iter().next().unwrap()
    }
}
