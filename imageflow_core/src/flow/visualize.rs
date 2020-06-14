use crate::Graph;
use crate::ffi::BitmapBgra;
use crate::internal_prelude::works_everywhere::*;
use super::definitions::{FrameEstimate, Node, PixelFormat, EdgeKind, NodeResult};


pub struct GraphRecordingInfo {
    pub debug_job_id: i32,
    pub current_graph_version: i32,
    pub record_graph_versions: bool,
    pub render_graph_versions: bool,
    pub maximum_graph_versions: i32,
}


pub struct GraphRecordingUpdate {
    pub next_graph_version: i32,
}

pub fn notify_graph_changed(graph_ref: &mut Graph,
                            r: &GraphRecordingInfo)
                            -> Result<Option<GraphRecordingUpdate>> {
    if !r.record_graph_versions || r.current_graph_version > r.maximum_graph_versions {
        return Ok(None);
        // println!("record_graph_versions=true, current_graph_version={}", current_graph_version);
    }
    if r.current_graph_version == 0 {
        job_delete_graphviz(r.debug_job_id).unwrap();
    }

    let prev_graph_version = r.current_graph_version - 1;

    let frame_prefix = format!("./node_frames/job_{}_node_", r.debug_job_id);

    let current_filename =
        format!("job_{}_graph_version_{}.dot", r.debug_job_id, r.current_graph_version);
    {
        let mut f = File::create(&current_filename).unwrap();
        print_graph(&mut f, graph_ref, Some(&frame_prefix)).unwrap();
        println!("Writing file {}", &current_filename);
    }
    if prev_graph_version >= 0 {
        let prev_filename =
            format!("job_{}_graph_version_{}.dot", r.debug_job_id, prev_graph_version);
        if files_identical(&current_filename, &prev_filename)
            .expect(&format!("Comparison err'd for {} and {}", &current_filename, &prev_filename)) {
            std::fs::remove_file(&current_filename).unwrap();

            // Next time we will overwrite the duplicate graph. The last two graphs may
            // remain dupes
            Ok(None)
        } else {
            if r.render_graph_versions {
                render_dotfile_to_png(&prev_filename)
            }
            Ok(Some(GraphRecordingUpdate { next_graph_version: r.current_graph_version + 1 }))
        }
    } else {
        Ok(Some(GraphRecordingUpdate { next_graph_version: r.current_graph_version + 1 }))
    }
}



pub fn render_dotfile_to_png(dotfile_path: &str) {
    let _ = std::process::Command::new("dot")
        .arg("-Tpng")
        .arg("-Gsize=11,16\\!")
        .arg("-Gdpi=150")
        .arg("-O")
        .arg(dotfile_path)
        .spawn();
    // .expect("dot command failed");
}
// pub fn job_render_graph_to_png(c: *mut Context, job: *mut Job, g: &mut Graph, graph_version: int32_t) -> bool
// {
//    let filename = format!("job_{}_graph_version_{}.dot", unsafe { (*job).debug_job_id }, graph_version);
//    let mut file = File::create(&filename).unwrap();
//    let _ = file.write_fmt(format_args!("{:?}", Dot::new(g.graph())));
//
//    return true;
// }



static INDENT: &'static str = "    ";

fn get_pixel_format_name_for(bitmap: *const BitmapBgra) -> &'static str {
    unsafe { get_pixel_format_name((*bitmap).fmt) }
}

fn get_pixel_format_name(fmt: PixelFormat) -> &'static str {
    match fmt {
        PixelFormat::Bgr24 => "bgra24",
        PixelFormat::Gray8 => "gray8",
        PixelFormat::Bgra32 => "bgra32",
        PixelFormat::Bgr32 => "bgr32",
        // _ => "?"
    }
}

pub fn print_graph(f: &mut dyn std::io::Write,
                   g: &Graph,
                   node_frame_filename_prefix: Option<&str>)
                   -> std::io::Result<()> {
    writeln!(f, "digraph g {{\n")?;
    writeln!(f, "{}node [shape=box, fontsize=20, fontcolor=\"#5AFA0A\" fontname=\"sans-serif bold\"]\n  size=\"12,18\"\n", INDENT)?;
    writeln!(f, "{}edge [fontsize=20, fontname=\"sans-serif\"]\n", INDENT)?;


    // output all edges
    for (i, edge) in g.raw_edges().iter().enumerate() {
        write!(f, "{}n{} -> n{}",
               INDENT,
               edge.source().index(),
               edge.target().index())?;

        let weight = g.node_weight(edge.source()).unwrap();

        let dimensions = match weight.result {
            NodeResult::Frame(ptr) => {
                unsafe {
                    format!("frame {}x{} {}", (*ptr).w, (*ptr).h, get_pixel_format_name_for(ptr))
                }
            }
            _ => {
                match weight.frame_est {
                    FrameEstimate::None => "?x?".to_owned(),
                    FrameEstimate::Some(info) => format!("est {}x{} {}", info.w, info.h, get_pixel_format_name(info.fmt)),
                    _ => "!x!".to_owned(),
                }
            }
        };
        write!(f, " [label=\"e{}: {}{}\"]\n", i, dimensions, match *g.edge_weight(EdgeIndex::new(i)).unwrap() {
            EdgeKind::Canvas => " canvas",
            _ => ""
        })?;
    }

    let mut total_ns: u64 = 0;

    // output all labels
    for index in g.graph().node_indices() {
        let weight: &Node = g.node_weight(index).unwrap();
        total_ns += u64::from(weight.cost.wall_ns);
        let ms = weight.cost.wall_ns as f64 / 1000f64;

        write!(f, "{}n{} [", INDENT, index.index())?;

        if let Some(prefix) = node_frame_filename_prefix {
            write!(f, "image=\"{}{}.png\", ", prefix, weight.stable_id)?;
        }
        write!(f, "label=\"n{}: ", index.index())?;
        weight.graphviz_node_label(f)?;
        write!(f, "\n{:.5}ms\"]\n", ms)?;
    }
    let total_ms = (total_ns as f64) / 1000.0f64;
    writeln!(f, "{}graphinfo [label=\"{} nodes\n{} edges\nExecution time: {:.3}ms\"]\n",
             INDENT, g.node_count(), g.edge_count(), total_ms)?;
    writeln!(f, "}}")?;
    Ok(())
}

fn remove_file_if_exists(path: &str) -> io::Result<()> {
    let result = std::fs::remove_file(path);
    if result.as_ref().err().and_then(|e| Some(e.kind() == io::ErrorKind::NotFound)) == Some(true) {
        return Ok(());
    }
    result
}
fn files_identical(filename_a: &str, filename_b: &str) -> std::io::Result<bool> {
    let mut a = File::open(filename_a)?;
    let mut a_str = Vec::new();
    a.read_to_end(&mut a_str)?;
    let mut b = File::open(filename_b)?;
    let mut b_str = Vec::new();
    b.read_to_end(&mut b_str)?;

    Ok(a_str == b_str)
}


fn job_delete_graphviz(job_id: i32) -> io::Result<()> {
    let safety_limit = 8000;

    // Keep deleting until we run out of files or hit a safety limit
    let mut node_index = 0;
    loop {
        let next = format!("./node_frames/job_{}_node_{}.png", job_id, node_index);
        if !Path::new(&next).exists() || node_index > safety_limit {
            break;
        } else {
            node_index += 1;
            remove_file_if_exists(&next)?;
        }
    }
    let mut version_index = 0;
    loop {
        let next = format!("job_{}_graph_version_{}.dot", job_id, version_index);
        let next_png = format!("job_{}_graph_version_{}.dot.png", job_id, version_index);
        let next_svg = format!("job_{}_graph_version_{}.dot.svg", job_id, version_index);
        if !Path::new(&next).exists() || version_index > safety_limit {
            break;
        } else {
            version_index += 1;
            remove_file_if_exists(&next)?;
            remove_file_if_exists(&next_png)?;
            remove_file_if_exists(&next_svg)?;
        }
    }
    Ok(())
}
