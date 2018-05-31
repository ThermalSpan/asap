
#[macro_use]
extern crate structopt;
extern crate cgmath;
#[macro_use]
extern crate glium;
extern crate aperture;
extern crate geoprim;
extern crate serde_json;
extern crate notify;
extern crate bincode;

use bincode::deserialize_from;
use std::thread;
use glium::glutin;
use glium::Surface;
use cgmath::prelude::*;
use std::time::{Duration, SystemTime};
use std::thread::sleep;
use std::fs::{File, metadata};
use std::io::prelude::*;
use geoprim::*;
use std::path::{Path, PathBuf};
use structopt::StructOpt;
use std::io::*;
use notify::{Watcher, RecursiveMode, RawEvent, raw_watcher};
use std::sync::mpsc::channel;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Args {
    /// Files to process
    #[structopt(name = "FILE", parse(from_os_str))]
    input: PathBuf,
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 3],
}
implement_vertex!(Vertex, position);

impl Vertex {
    fn from(p: Point) -> Vertex {
        Vertex { position: [p.x, p.y, p.z] }
    }
}


fn get_last_mod(path: &Path) -> Option<SystemTime> {
    // Sigh
    // First we get the attributes...
    let file_attributes;
    match metadata(&path) {
        Ok(attr) => {
            file_attributes = attr;
        }
        Err(e) => {
            println!(
                "ERROR: unable to get file metadata for {}:\n{}",
                path.display(),
                e
            );
            return None;
        }
    }

    // Then, does the attributes have the modified time?
    let last_mod;
    match file_attributes.modified() {
        Ok(time) => {
            last_mod = time;
        }
        Err(e) => {
            println!(
                "ERROR: unable to get modified time for {}\n{}",
                path.display(),
                e
            );
            return None;
        }
    }

    Some(last_mod)
}

fn plot_to_buffers<F>(
    plot: &Plot,
    display: &F,
) -> (glium::VertexBuffer<Vertex>, glium::IndexBuffer<u32>)
where
    F: glium::backend::Facade,
{
    let mut vertices = Vec::new();
    let mut linelist = Vec::new();

    for i in 0..plot.lines.len() {
        let line = plot.lines[i];
        vertices.push(Vertex::from(line.p1));
        vertices.push(Vertex::from(line.p2));

        linelist.push((2 * i) as u32);
        linelist.push((2 * i + 1) as u32);
    }

    let vertex_buffer = glium::VertexBuffer::new(display, &vertices).unwrap();
    let index_buffer =
        glium::IndexBuffer::new(display, glium::index::PrimitiveType::LinesList, &linelist)
            .unwrap();

    (vertex_buffer, index_buffer)
}

fn points_to_buffers<F>(
    plot: &Plot,
    display: &F,
) -> (glium::VertexBuffer<Vertex>, glium::IndexBuffer<u32>)
where
    F: glium::backend::Facade,
{
    let mut vertices = Vec::new();
    let mut linelist = Vec::new();

    for i in 0..plot.points.len() {
        let p = plot.points[i];
        vertices.push(Vertex::from(p));
        linelist.push(i as u32);
    }

    let vertex_buffer = glium::VertexBuffer::new(display, &vertices).expect("Cant vertex buffer");
    let index_buffer =
        glium::IndexBuffer::new(display, glium::index::PrimitiveType::Points, &linelist)
            .expect("Can't index_buffer it");

    (vertex_buffer, index_buffer)
}

fn main() {
    let args = Args::from_args();

    let mut events_loop = glutin::EventsLoop::new();
    let window = glutin::WindowBuilder::new()
        .with_title("ASAP")
        .with_dimensions(1024, 1024);
    let context = glutin::ContextBuilder::new();
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    // We statically include the shader sources, and build the shader program
    let vertex_shader_src = include_str!("_shaders/vertex_shader.vert");
    let fragment_shader_src = include_str!("_shaders/fragment_shader.frag");
    let shader_program =
        glium::Program::from_source(&display, vertex_shader_src, fragment_shader_src, None)
            .unwrap();

    let mut cam = aperture::Camera::new();
    // Drawing parameters
    let params = glium::DrawParameters {
        line_width: Some(5.0),
        point_size: Some(10.0),
        depth: glium::draw_parameters::Depth {
            test: glium::DepthTest::IfMoreOrEqual,
            ..Default::default()
        },
        blend: glium::Blend::alpha_blending(),
        ..Default::default()
    };



    let input_file = File::open(&args.input).unwrap();
    let mut reader = BufReader::new(input_file);
    let plot: Plot = deserialize_from(&mut reader).unwrap();
    let mut linebuffers = plot_to_buffers(&plot, &display);
    let mut pointbuffers = points_to_buffers(&plot, &display);
    let mut lastmod = get_last_mod(&args.input).unwrap();

    let mut closed = false;
    let fps: f32 = 60.0;
    let frame_duration_cap = Duration::from_millis(((1.0 / fps) * 1000.0) as u64);
    let mut current_time = SystemTime::now();
    while !closed {
        let mut target = display.draw();
        // listing the events produced by application and waiting to be received
        events_loop.poll_events(|ev| match ev {
            glutin::Event::WindowEvent { event: glutin::WindowEvent::Closed, .. } => {
                closed = true;
            }
            event => {
                aperture::camera_event_handler(&mut cam, event);
            }
        });

        if let Some(last_mod_time) = get_last_mod(&args.input) {
            if last_mod_time > lastmod {
                let input_file = File::open(&args.input).unwrap();
                let mut reader = BufReader::new(input_file);
                let plot: Plot = serde_json::from_reader(&mut reader).expect("Can't json it");
                linebuffers = plot_to_buffers(&plot, &display);
                pointbuffers = points_to_buffers(&plot, &display);
                lastmod = last_mod_time;
                println!("Updated plot");
            }
        }

        let new_time = SystemTime::now();
        let frame_time = current_time.elapsed().unwrap();
        let elapsed_millis = (1000 * frame_time.as_secs() + frame_time.subsec_millis() as u64) as
            f32;
        current_time = new_time;

        let (window_width, window_height) = {
            let (window_width_i, window_height_i) = target.get_dimensions();
            (window_width_i as f32, window_height_i as f32)
        };

        cam.update(elapsed_millis, window_width, window_height);

        let world_transform: [[f32; 4]; 4] = cam.get_clipspace_transform().into();

        // A weird yellow background
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        let uniforms =
            uniform!{
            object_transform: world_transform,
            u_color: [1.0, 1.0, 1.0, 1.0f32]
        };

        // Clear the screen, draw, and swap the buffers
        target
            .draw(
                &linebuffers.0,
                &linebuffers.1,
                &shader_program,
                &uniforms,
                &params,
            )
            .unwrap();

        let uniforms_p =
            uniform!{
            object_transform: world_transform,
            u_color: [0.6, 0.0, 0.17, 1.0f32]
        };

        // Clear the screen, draw, and swap the buffers
        target
            .draw(
                &pointbuffers.0,
                &pointbuffers.1,
                &shader_program,
                &uniforms_p,
                &params,
            )
            .unwrap();


        // Here we limit the framerate to avoid consuming uneeded CPU time
        let elapsed = current_time.elapsed().unwrap();
        if elapsed < frame_duration_cap {
            let sleep_time = frame_duration_cap - elapsed;
            sleep(sleep_time);
        }

        target.finish().unwrap();
    }
}
