use crate::scene::WasmSceneController;
use anyhow::anyhow;
use dioxus::prelude::use_future;
use dioxus::prelude::*;
use dioxus_html_macro::html;
use glam::{uvec2, Vec2};
use inox2d::{formats::inp::parse_inp, render::opengl::OpenglRenderer};
use std::cell::RefCell;
use std::rc::Rc;
use tracing::info;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use web_sys::Window;
use winit::event::{Event, WindowEvent};

use winit::dpi::PhysicalSize;
use winit::platform::web::WindowBuilderExtWebSys;
use winit::platform::web::WindowExtWebSys;
use winit::window::WindowBuilder;
use bytes::Bytes;
use crate::base_url;
use crate::request_animation_frame;

#[inline_props]
pub fn inox2d_component<'a>(cx: Scope, href: &'a str, width: u32, height: u32) -> Element<'a> {
    let uri = href.to_string();
    /*let puppet = use_future(cx, (), |()| async move {
        let res = reqwest::Client::new()
        .get(uri)
        .send()
        .await?
        .bytes()
        .await.unwrap();

        return parse_inp(res.as_ref()).unwrap();
    });*/

    let renderer = use_future(cx, (), |()| async move {
        runwrap().await
    });
    cx.render(html!(
<div class="flex justify-center items-center h-screen">
  <div class="relative w-1/2">
    <canvas id="canvas" width="{width}" height="{height}" tabindex="0" class="border-4 border-gray-500 rounded-lg aspect-w-1 aspect-h-2"></canvas>
      <div class="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 w-3/4">
        //<div class="bg-blue-500 h-2 rounded-full"></div>
      </div>
    </div>
    <div class="absolute bottom-0 left-1/2 transform -translate-x-1/2 mb-6 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
      <button class="w-8 h-8 rounded-full bg-red-500 mx-1">
        <span class="sr-only">"Reset"</span>
      </button>
      <button class="w-8 h-8 rounded-full bg-green-500 mx-1">
        <span class="sr-only">"Follow mouse with eyes"</span>
      </button>
    </div>
  </div>
    ))
}

async fn run() -> anyhow::Result<()> {
    let canvas = get_canvas_element().ok_or(anyhow!("Couldn't find canvas"))?;

    let canvas_size = (canvas.width(), canvas.height());

    let events = winit::event_loop::EventLoop::new();
    let window = WindowBuilder::new()
        .with_canvas(Some(canvas.clone()))
        .with_resizable(false)
        .with_inner_size(PhysicalSize::new(canvas_size.0, canvas_size.1))
        .build(&events)?;

    let gl = setup_wgl2(canvas)?;

    info!("Loading puppet");
    let res = reqwest::Client::new()
        .get(format!("{}/assets/arch-chan.inp"))
        .send()
        .await?;

    let model_bytes = res.bytes().await?;
    let model = parse_inp(model_bytes.as_ref())?;
    let puppet = model.puppet;

    info!("Initializing Inox2D renderer");
    let window_size = window.inner_size();
    let viewport = uvec2(window_size.width, window_size.height);
    let mut renderer = OpenglRenderer::new(gl, viewport, &puppet)?;

    info!("Uploading model textures");
    renderer.upload_model_textures(&model.textures)?;
    renderer.camera.scale = Vec2::splat(0.15);
    info!("Inox2D renderer initialized");

    let scene_ctrl = WasmSceneController::new(&renderer.camera, 0.5);

    // Refcells because we need to make our own continuous animation loop.
    // Winit won't help us :(
    let scene_ctrl = Rc::new(RefCell::new(scene_ctrl));
    let renderer = Rc::new(RefCell::new(renderer));
    let puppet = Rc::new(RefCell::new(puppet));

    // Setup continuous animation loop
    {
        let anim_loop_f = Rc::new(RefCell::new(None));
        let anim_loop_g = anim_loop_f.clone();
        let scene_ctrl = scene_ctrl.clone();
        let renderer = renderer.clone();
        let puppet = puppet.clone();

        *anim_loop_g.borrow_mut() = Some(Closure::new(move || {
            scene_ctrl
                .borrow_mut()
                .update(&mut renderer.borrow_mut().camera);

            renderer.borrow().clear();
            {
                let mut puppet = puppet.borrow_mut();
                puppet.begin_set_params();
                let t = scene_ctrl.borrow().current_elapsed();
                //puppet.set_param("Head:: Yaw-Pitch", Vec2::new(t.cos(), t.sin()));
                puppet.end_set_params();
            }
            renderer.borrow().render(&puppet.borrow());

            request_animation_frame(anim_loop_f.borrow().as_ref().unwrap());
        }));
        request_animation_frame(anim_loop_g.borrow().as_ref().unwrap());
    }

    // Event loop
    events.run(move |event, _, control_flow| {
        // it needs to be present
        let _window = &window;

        control_flow.set_wait();

        match event {
            Event::WindowEvent { ref event, .. } => match event {
                WindowEvent::Resized(physical_size) => {
                    // Handle window resizing
                    renderer
                        .borrow_mut()
                        .resize(physical_size.width, physical_size.height);
                    window.request_redraw();
                }
                WindowEvent::CloseRequested => control_flow.set_exit(),
                _ => scene_ctrl
                    .borrow_mut()
                    .interact(&window, event, &renderer.borrow().camera),
            },
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => (),
        }
    })
}

async fn runwrap() {
    match run().await {
        Ok(_) => tracing::info!("Shutdown"),
        Err(e) => tracing::error!("Fatal crash: {}", e),
    }
}

fn get_canvas_element() -> Option<HtmlCanvasElement> {
    match web_sys::window() {
        None => None,
        Some(w) => match w.document() {
            None => None,
            Some(d) => match d.get_element_by_id("canvas") {
                None => None,
                Some(c) => c.dyn_into::<web_sys::HtmlCanvasElement>().ok(),
            },
        },
    }
}

fn setup_wgl2(canvas: HtmlCanvasElement) -> anyhow::Result<glow::Context> {
    let context_options = js_sys::Object::new();

    // make sure context has a stencil buffer
    match js_sys::Reflect::set(&context_options, &"stencil".into(), &true.into()) {
        Ok(_) => Ok(()),
        Err(e) => {
            let s = e
                .as_string()
                .ok_or(anyhow!("Context doesn't have a stencil buffer"))?;

            Err(anyhow!("Context doesn't have a stencil buffer").context(s))
        }
    }?;

    let wgl2 = match canvas.get_context_with_context_options("webgl2", &context_options) {
        Ok(Some(v)) => match v.dyn_into::<web_sys::WebGl2RenderingContext>() {
            Ok(c) => Ok(c),
            Err(e) => {
                let s = e
                    .as_string()
                    .ok_or(anyhow!("Error creating webgl2 context"))?;

                Err(anyhow!("Error creating webgl2 context").context(s))
            }
        },
        Ok(None) => Err(anyhow!("Error creating webgl2 context")),
        Err(e) => {
            let s = e
                .as_string()
                .ok_or(anyhow!("Error creating webgl2 context"))?;

            Err(anyhow!("Error creating webgl2 context").context(s))
        }
    }?;

    Ok(glow::Context::from_webgl2_context(wgl2))
}
