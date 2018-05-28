
#[macro_use]
extern crate structopt;
extern crate cgmath;
#[macro_use]
extern crate glium;
extern crate aperture;
extern crate geoprim;
extern crate serde_json;

use glium::glutin;
use glium::Surface;
use cgmath::prelude::*;
use std::time::{Duration, SystemTime};
use std::thread::sleep;
use std::fs::File;
use std::io::prelude::*;
use geoprim::*;
use std::path::PathBuf;
use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Args {
    /// Files to process
    #[structopt(name = "FILE", parse(from_os_str))]
    input: PathBuf
}

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 3],
}
implement_vertex!(Vertex, position);

impl Vertex {
    fn from(p: Point) -> Vertex {
        Vertex {
            position: [p.x, p.y, p.z]
        }
    }
}

fn plot_to_buffers<F> (plot: &Plot, display: &F) -> (glium::VertexBuffer<Vertex>, glium::IndexBuffer<u16>) where F: glium::backend::Facade  {
    let mut vertices = Vec::new();
    let mut linelist = Vec::new();

    for i in 0..plot.lines.len() {
        let line = plot.lines[i];
        vertices.push(Vertex::from(line.p1));
        vertices.push(Vertex::from(line.p2));
        
        linelist.push((2 * i) as u16);
        linelist.push((2 * i + 1) as u16);
    }

    let vertex_buffer = glium::VertexBuffer::new(display, &vertices).unwrap();
    let index_buffer = glium::IndexBuffer::new(
        display,
        glium::index::PrimitiveType::LinesList,
        &linelist
    ).unwrap();

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

    // Drawing parameters
    let params = glium::DrawParameters {
        line_width: Some(5.0),
        blend: glium::Blend::alpha_blending(),
        ..Default::default()
    };

    let mut input_file = File::open(args.input).unwrap();
    let plot: Plot = serde_json::from_reader(&input_file).unwrap();
    let (vertex_buffer, indices) = plot_to_buffers(&plot, &display);

    let mut closed = false;
    let mut cam = aperture::Camera::new();
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
                &vertex_buffer,
                &indices,
                &shader_program,
                &uniforms,
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
