#[macro_use]
extern crate structopt;
extern crate cgmath;
#[macro_use]
extern crate glium;
extern crate aperture;
extern crate bincode;
extern crate geoprim;
extern crate notify;
extern crate serde_json;

use bincode::deserialize_from;
use cgmath::prelude::*;
use geoprim::*;
use glium::glutin;
use glium::Surface;
use notify::{raw_watcher, RawEvent, RecursiveMode, Watcher};
use std::fs::{metadata, File};
use std::io::prelude::*;
use std::io::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use structopt::StructOpt;

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
        Vertex {
            position: [p.x, p.y, p.z],
        }
    }
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

    // Create a channel to receive the events.
    let (event_sender, event_reciever) = channel();
    let mut watcher = notify::watcher(event_sender, Duration::from_millis(500)).unwrap();
    watcher
        .watch(&args.input, notify::RecursiveMode::NonRecursive)
        .unwrap();

    let input_file = File::open(&args.input).unwrap();
    let mut reader = BufReader::new(input_file);
    let plot: Plot = deserialize_from(&mut reader).unwrap();
    let mut linebuffers = plot_to_buffers(&plot, &display);
    let mut pointbuffers = points_to_buffers(&plot, &display);

    let mut closed = false;
    let fps: f32 = 60.0;
    let frame_duration_cap = Duration::from_millis(((1.0 / fps) * 1000.0) as u64);
    let mut current_time = SystemTime::now();
    while !closed {
        let mut target = display.draw();
        // listing the events produced by application and waiting to be received
        events_loop.poll_events(|ev| match ev {
            glutin::Event::WindowEvent {
                event: glutin::WindowEvent::Closed,
                ..
            } => {
                closed = true;
            }
            event => {
                aperture::camera_event_handler(&mut cam, event);
            }
        });

        while let Ok(event) = event_reciever.try_recv() {
            match event {
                notify::DebouncedEvent::Write(p) | notify::DebouncedEvent::Create(p) => {
                    let input_file = File::open(&p).unwrap();
                    let mut reader = BufReader::new(input_file);
                    let plot: Plot = deserialize_from(&mut reader).expect("Can't deser it");
                    linebuffers = plot_to_buffers(&plot, &display);
                    pointbuffers = points_to_buffers(&plot, &display);
                }
                _ => (),
            }
        }

        let new_time = SystemTime::now();
        let frame_time = current_time.elapsed().unwrap();
        let elapsed_millis =
            (1000 * frame_time.as_secs() + frame_time.subsec_millis() as u64) as f32;
        current_time = new_time;

        let (window_width, window_height) = {
            let (window_width_i, window_height_i) = target.get_dimensions();
            (window_width_i as f32, window_height_i as f32)
        };

        cam.update(elapsed_millis, window_width, window_height);

        let world_transform: [[f32; 4]; 4] = cam.get_clipspace_transform().into();

        // A weird yellow background
        target.clear_color(0.0, 0.0, 0.0, 0.0);

        let uniforms = uniform!{
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

        let uniforms_p = uniform!{
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
