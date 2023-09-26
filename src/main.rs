use image::Pixel;
use imageproc::drawing::{Blend, Canvas};
use std::borrow::Cow;

use rusttype::{point, vector, Font, PositionedGlyph, Rect, Scale};
use wgpu::util::DeviceExt;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod input;

// In WGPU, we define an async function whose operation can be suspended and resumed.
// This is because on web, we can't take over the main event loop and must leave it to
// the browser.  On desktop, we'll just be running this function to completion.
async fn run(event_loop: EventLoop<()>, window: Window) {
    let size = window.inner_size();

    // An Instance is an instance of the graphics API.  It's the context in which other
    // WGPU values and operations take place, and there can be only one.
    // Its implementation of the Default trait automatically selects a driver backend.
    let instance = wgpu::Instance::default();

    // From the OS window (or web canvas) the graphics API can obtain a surface onto which
    // we can draw.  This operation is unsafe (it depends on the window not outliving the surface)
    // and it could fail (if the window can't provide a rendering destination).
    // The unsafe {} block allows us to call unsafe functions, and the unwrap will abort the program
    // if the operation fails.
    let surface = unsafe { instance.create_surface(&window) }.unwrap();

    // Next, we need to get a graphics adapter from the instance---this represents a physical
    // graphics card (GPU) or compute device.  Here we ask for a GPU that will be able to draw to the
    // surface we just obtained.
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        // This operation can take some time, so we await the result. We can only await like this
        // in an async function.
        .await
        // And it can fail, so we panic with an error message if we can't get a GPU.
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue.  A logical device is like a connection to a GPU, and
    // we'll be issuing instructions to the GPU over the command queue.
    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                // We don't need to ask for any optional GPU features for our simple example
                features: wgpu::Features::empty(),
                // Make sure we use very broadly compatible limits for our driver,
                // and also use the texture resolution limits from the adapter.
                // This is important for supporting images as big as our swapchain.
                limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
            },
            None,
        )
        // request_device is also an async function, so we need to wait for the result.
        .await
        .expect("Failed to create device");

    // The swapchain is how we obtain images from the surface we're drawing onto.
    // This is so we can draw onto one image while a different one is being presented
    // to the user on-screen.
    let swapchain_capabilities = surface.get_capabilities(&adapter);
    // We'll just use the first supported format, we don't have any reason here to use
    // one format or another.
    let swapchain_format = swapchain_capabilities.formats[0];

    // Our surface config lets us set up our surface for drawing with the device
    // we're actually using.  It's mutable in case the window's size changes later on.
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: swapchain_capabilities.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    // Load the shaders from disk.  Remember, shader programs are things we compile for
    // our GPU so that it can compute vertices and colorize fragments.
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        // Cow is a "copy on write" wrapper that abstracts over owned or borrowed memory.
        // Here we just need to use it since wgpu wants "some text" to compile a shader from.
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });
    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            // This bind group's first entry is for the texture and the second is for the sampler.
            entries: &[
                // The texture binding
                wgpu::BindGroupLayoutEntry {
                    // This matches the binding number in the shader
                    binding: 0,
                    // Only available in the fragment shader
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    // It's a texture binding
                    ty: wgpu::BindingType::Texture {
                        // We can use it with float samplers
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        // It's being used as a 2D texture
                        view_dimension: wgpu::TextureViewDimension::D2,
                        // This is not a multisampled texture
                        multisampled: false,
                    },
                    // This is not an array texture, so it has None for count
                    count: None,
                },
                // The sampler binding
                wgpu::BindGroupLayoutEntry {
                    // This matches the binding number in the shader
                    binding: 1,
                    // Only available in the fragment shader
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    // It's a sampler
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    // No count
                    count: None,
                },
            ],
        });
    use std::path::Path;
    let img = image::open(Path::new(
        "/Users/calebtheperson/RustProjects/triangle/src/47.png",
    ))
    .expect("Bruh where ur picture'");
    let img = img.to_rgba8();
    let (img_w, img_h) = img.dimensions();
    // How big is the texture in GPU memory?
    let size = wgpu::Extent3d {
        width: img_w,
        height: img_h,
        depth_or_array_layers: 1,
    };

    // Let's make a texture now
    let texture = device.create_texture(
        // Parameters for the texture
        &wgpu::TextureDescriptor {
            // An optional label
            label: Some("47 image"),
            // Its dimensions. This line is equivalent to size:size
            size,
            // Number of mipmapping levels (to show different pictures at different distances)
            mip_level_count: 1,
            // Number of samples per pixel in the texture. It'll be one for our whole class.
            sample_count: 1,
            // Is it a 1D, 2D, or 3D texture?
            dimension: wgpu::TextureDimension::D2,
            // 8 bits per component, four components per pixel, unsigned, normalized in 0..255, SRGB
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            // This texture will be bound for shaders and have stuff copied to it
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            // What formats are allowed as views on this texture besides the native format
            view_formats: &[],
        },
    );
    // Now that we have a texture, we need to copy its data to the GPU:
    queue.write_texture(
        // A description of where to write the image data.
        // We'll use this helper to say "the whole texture"
        texture.as_image_copy(),
        // The image data to write
        &img,
        // What portion of the image data to copy from the CPU
        wgpu::ImageDataLayout {
            // Where in img do the bytes to copy start?
            offset: 0,
            // How many bytes in each row of the image?
            bytes_per_row: Some(4 * img_w),
            // We could pass None here and it would be alright,
            // since we're only uploading one image
            rows_per_image: Some(img_h),
        },
        // What portion of the texture we're writing into
        size,
    );

    // AsRef means we can take as parameters anything that cheaply converts into a Path,
    // for example an &str.
    fn load_texture(
        path: impl AsRef<std::path::Path>,
        label: Option<&str>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(wgpu::Texture, image::RgbaImage), image::ImageError> {
        // This ? operator will return the error if there is one, unwrapping the result otherwise.
        let img = image::open(path.as_ref())?.to_rgba8();
        let (width, height) = img.dimensions();
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            &img,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        Ok((texture, img))
    }
    let (tex_47, mut img_47) = load_texture(
        "/Users/calebtheperson/RustProjects/interactive-drawing/src/47.png",
        Some("47 image"),
        &device,
        &queue,
    )
    .expect("Couldn't load 47 img");
    let view_47 = tex_47.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler_47 = device.create_sampler(&wgpu::SamplerDescriptor::default());
    let tex_47_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &texture_bind_group_layout,
        entries: &[
            // One for the texture, one for the sampler
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view_47),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler_47),
            },
        ],
    });

    // A graphics pipeline is sort of like the conventions for a function call: it defines
    // the shapes of arguments (bind groups and push constants) that will be used for
    // draw calls.
    // Now we'll create our pipeline layout, specifying the shape of the execution environment (the bind group)
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&texture_bind_group_layout],
        push_constant_ranges: &[],
    });

    // Our specific "function" is going to be a draw call using our shaders. That's what we
    // set up here, calling the result a render pipeline.  It's not only what shaders to use,
    // but also how to interpret streams of vertices (e.g. as separate triangles or as a list of lines),
    // whether to draw both the fronts and backs of triangles, and how many times to run the pipeline for
    // things like multisampling antialiasing.
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(swapchain_format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    // Now our setup is all done and we can kick off the windowing event loop.
    // This closure is a "move closure" that claims ownership over variables used within its scope.
    // It is called once per iteration of the event loop.

    let mut input = input::Input::default();
    let mut color = image::Rgba([255, 0, 0, 255]);
    let mut brush_size = 10_i32;
    let mut alphaValue: u8 = 16;
    event_loop.run(move |event, _, control_flow| {
        // By default, tell the windowing system that there's no more work to do
        // from the application's perspective.
        *control_flow = ControlFlow::Wait;
        // Depending on the event, we'll need to do different things.
        // There is some pretty fancy pattern matching going on here,
        // so think back to CSCI054.
        match event {
            Event::WindowEvent {
                // For example, "if it's a window event and the specific window event is that
                // we have resized the window to a particular new size called `size`..."
                event: WindowEvent::Resized(size),
                // Ignoring the rest of the fields of Event::WindowEvent...
                ..
            } => {
                // Reconfigure the surface with the new size
                config.width = size.width;
                config.height = size.height;
                surface.configure(&device, &config);
                // On MacOS the window needs to be redrawn manually after resizing
                window.request_redraw();
            }
            // WindowEvent->KeyboardInput: Keyboard input!
            Event::WindowEvent {
                // Note this deeply nested pattern match
                event: WindowEvent::KeyboardInput { input: key_ev, .. },
                ..
            } => {
                input.handle_key_event(key_ev);
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                ..
            } => {
                input.handle_mouse_button(state, button);
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                input.handle_mouse_move(position);
            }

            Event::RedrawRequested(_) => {
                input.next_frame();

                let img_47_w = img_47.width();
                let img_47_h = img_47.height();

                // Your turn: Use the number keys 1-3 to change the color...
                // (1)
                // <YOUR CODE HERE>
                //This code changes the Color
                if input.is_key_down(winit::event::VirtualKeyCode::Key1) {
                    color = image::Rgba([128, 0, 128, alphaValue]);
                } else if input.is_key_down(winit::event::VirtualKeyCode::Key2) {
                    color = image::Rgba([0, 0, 125, alphaValue]);
                } else if input.is_key_down(winit::event::VirtualKeyCode::Key3) {
                    color = image::Rgba([17, 49, 12, alphaValue]);
                }

                //This code changes the transparency
                if input.is_key_down(winit::event::VirtualKeyCode::Comma) {
                    if alphaValue > 5 {
                        alphaValue -= 5;
                    }
                    color = image::Rgba([color[0], color[1], color[2], alphaValue]);
                } else if input.is_key_down(winit::event::VirtualKeyCode::Period) {
                    if alphaValue < 250 {
                        alphaValue += 5;
                    }
                    color = image::Rgba([color[0], color[1], color[2], alphaValue]);
                }
                // And use the numbers 9 and 0 to change the brush size:
                if input.is_key_down(winit::event::VirtualKeyCode::Key9) {
                    brush_size = (brush_size - 1).clamp(1, 50);
                } else if input.is_key_down(winit::event::VirtualKeyCode::Key0) {
                    brush_size = (brush_size + 1).clamp(1, 50);
                }
                // Here's how we'll splatter paint on the 47 image:
                if input.is_mouse_down(winit::event::MouseButton::Left) {
                    let mouse_pos = input.mouse_pos();
                    // (2)
                    let (mouse_x_norm, mouse_y_norm) = (
                        (mouse_pos.x / config.width as f64), // Divide it by widhth and height of the screen to get normalized positiions ?
                        (mouse_pos.y / config.height as f64),
                    );

                    let mut img_data = img_47.as_flat_samples_mut();
                    let mut blend = imageproc::drawing::Blend(img_data.as_view_mut().unwrap());

                    // imageproc::drawing::draw_filled_circle_mut(
                    //     &mut blend,
                    //     (
                    //         (mouse_x_norm * (img_47_w as f64)) as i32,
                    //         (mouse_y_norm * (img_47_h as f64)) as i32,
                    //     ),
                    //     brush_size,
                    //     color,
                    // );

                    //Rectangle 1
                    // imageproc::drawing::draw_filled_rect_mut(
                    //     &mut blend,
                    //     imageproc::rect::Rect::at(
                    //         ((mouse_x_norm * (img_47_w as f64)) as i32),
                    //         ((mouse_y_norm * (img_47_h as f64)) as i32),
                    //     )
                    //     .of_size((brush_size as u32), (brush_size as u32)),
                    //     color,
                    // );

                    //Outlined Polygons 2
                    // imageproc::drawing::draw_hollow_ellipse_mut(
                    //     &mut blend,
                    //     (
                    //         (mouse_x_norm * (img_47_w as f64)) as i32,
                    //         (mouse_y_norm * (img_47_h as f64)) as i32,
                    //     ),
                    //     brush_size,
                    //     brush_size,
                    //     color,
                    // );

                    let font_data = include_bytes!("../src/Harlow Solid Italic Italic.ttf");
                    let font = Font::try_from_bytes(font_data);
                    imageproc::drawing::draw_text_mut(
                        &mut blend,
                        color,
                        ((mouse_x_norm * (img_47_w as f64)) as i32),
                        ((mouse_y_norm * (img_47_h as f64)) as i32),
                        Scale {
                            x: (brush_size as f32),
                            y: (brush_size as f32),
                        },
                        &font.unwrap(),
                        "POG",
                    );

                    // We've modified the image in memory---now to update the texture!
                    // This queues up a texture copy for later, copying the image data.
                    queue.write_texture(
                        // Telling our
                        tex_47.as_image_copy(),
                        &img_47,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * img_47_w),
                            rows_per_image: Some(img_47_h),
                        },
                        wgpu::Extent3d {
                            width: img_47_w,
                            height: img_47_h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                // Remember this from before?
                input.next_frame(); //WE are done processing inputs for right now

                // ... All the 3d drawing code/render pipeline/queue/frame stuff goes here ...

                // If the window system is telling us to redraw, let's get our next swapchain image
                let frame = surface
                    .get_current_texture()
                    .expect("Failed to acquire next swap chain texture");
                // And set up a texture view onto it, since the GPU needs a way to interpret those
                // image bytes for writing.
                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                // From the queue we obtain a command encoder that lets us issue GPU commands
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    // Now we begin a render pass.  The descriptor tells WGPU that
                    // we want to draw onto our swapchain texture view (that's where the colors will go)
                    // and that there's no depth buffer or stencil buffer.
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                // When loading this texture for writing, the GPU should clear
                                // out all pixels to a lovely green color
                                load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                // The results of drawing should always be stored to persistent memory
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });

                    rpass.set_bind_group(0, &tex_47_bind_group, &[]);

                    rpass.set_pipeline(&render_pipeline);

                    // Attach the bind group for group 0
                    // Now draw two triangles!
                    rpass.draw(0..6, 0..1);
                }
                // Once the commands have been scheduled, we send them over to the GPU via the queue.
                queue.submit(Some(encoder.finish()));
                // Then we wait for the commands to finish and tell the windowing system to
                // present the swapchain image.
                frame.present();

                // (3)
                // And we have to tell the window to redraw!
                window.request_redraw(); // Creates a loop and procedds to redraw the window
            }
            // If we're supposed to close the window, tell the event loop we're all done
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            // Ignore every other event for now.
            _ => {}
        }
    });
}

// Main is just going to configure an event loop, open a window, set up logging,
// and kick off our `run` function.
fn main() {
    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        // On native, we just want to wait for `run` to finish.
        pollster::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        // On web things are a little more complicated.
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("could not initialize logger");
        use winit::platform::web::WindowExtWebSys;
        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");
        // Now we use the browser's runtime to spawn our async run function.
        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}
