use crate::scene::WasmSceneController;
use anyhow::anyhow;
use dioxus::prelude::use_future;
use dioxus::prelude::*;
use dioxus_html_macro::html;
use glam::Vec2;
use inox2d::model::Model;
use inox2d::formats::inp::parse_inp;
use inox2d::render::InoxRenderer;
use inox2d_opengl::OpenglRenderer;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::info;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::platform::web::WindowBuilderExtWebSys;
use winit::window::WindowBuilder;
use inox2d::puppet::PuppetMeta;

#[inline_props]
pub fn inox2d_component<'a>(cx: Scope, href: &'a str, width: u32, height: u32) -> Element<'a> {
    let uri = href.to_string();

    let renderer = use_coroutine(
        cx,
        |mut model_channel: UnboundedReceiver<Model>| async move { runwrap(&mut model_channel).await },
    );

    let puppet_meta = use_future(cx, (renderer,), |(renderer,)| async move {
        info!("Downloading model...");

        let res = reqwest::Client::new()
            .get(uri)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();

        info!("Model received");

        let model = parse_inp(res.as_ref()).unwrap();
        info!("Model parsed");

        let meta = model.puppet.meta.clone();

        renderer.send(model);
        info!("Model sent");

        meta
    });

    cx.render(html!(
        <div class="flex justify-center items-center flex-row m-2 max-w-sm">
            <div class="flex justify-center items-center flex-col m-2 max-w-sm">
                <canvas id="canvas" width="{width}" height="{height}" tabindex="0" class="border-4 border-gray-500 rounded-lg aspect-w-1 aspect-h-2"></canvas>
                    //<div class="absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 w-3/4">
                        //<div class="bg-blue-500 h-2 rounded-full"></div>
                    //</div>
                <div class="m-2 flex flex-row w-full">
                    <button class="w-1/2 h-8 rounded-lg border-4 border-gray-500 mx-1">
                        <span class="">"Reset"</span>
                    </button>
                    <button class="w-1/2 h-8 rounded-lg border-4 border-gray-500 mx-1">
                        <span class="">"Follow mouse with eyes"</span>
                    </button>
                </div>
            </div>
            <div class="w-1/2 h-8 rounded-lg border-4 border-gray-500 mx-1">
                <puppet_meta_info meta={puppet_meta.value()}></puppet_meta_info>
            </div>
        </div>
    ))
}

#[inline_props]
fn puppet_meta_info<'a>(cx: Scope, meta: Option<Option<&'a PuppetMeta>>) -> Element {
    cx.render(match meta.flatten() {
        Some(m) => html!(
            <dl>
                <puppet_meta_field field_name={"Name"} field={m.name.clone()}></puppet_meta_field>
                <puppet_meta_field field_name={"Artist"} field={m.artist.clone()}></puppet_meta_field>
                <puppet_meta_field field_name={"Rigger"} field={m.rigger.clone()}></puppet_meta_field>
                <puppet_meta_field field_name={"Name"} field={m.name.clone()}></puppet_meta_field>
                <dt>"Version"</dt>
                <dd>{m.version.clone()}</dd>
                <puppet_meta_field field_name={"Copyright"} field={m.copyright.clone()}></puppet_meta_field>
                <puppet_meta_field field_name={"License URL"} field={m.license_url.clone()}></puppet_meta_field>
                <puppet_meta_field field_name={"Contact"} field={m.contact.clone()}></puppet_meta_field>
                <puppet_meta_field field_name={"Reference"} field={m.reference.clone()}></puppet_meta_field>
            </dl>
        ),
        None => html!(
            <div>
            </div>
        )
    })
}

#[inline_props]
fn puppet_meta_field(cx: Scope, field_name: &'static str, field: Option<Option<String>>) -> Element {
    cx.render(match field.clone().flatten() {
        Some(m) => rsx!(
            dt {
                field_name.clone()
            }
            dd {
                m
            }
        ),
        None => rsx!(
            div {}
        )
    })
}

async fn run(model_channel: &mut UnboundedReceiver<Model>) -> anyhow::Result<()> {
    info!("Initializing canvas...");
    let canvas = get_canvas_element().ok_or(anyhow!("Couldn't find canvas"))?;

    let canvas_size = (canvas.width(), canvas.height());

    let events = winit::event_loop::EventLoop::new();
    let window = WindowBuilder::new()
        .with_canvas(Some(canvas.clone()))
        .with_resizable(false)
        .with_inner_size(PhysicalSize::new(canvas_size.0, canvas_size.1))
        .build(&events)?;

    let gl = setup_wgl2(canvas)?;

    info!("Waiting for model...");

    let model = wait_for_model(model_channel).await?;

    info!("== Puppet Meta ==\n{}", &model.puppet.meta);
    info!("== Nodes ==\n{}", &model.puppet.nodes);
    if model.vendors.is_empty() {
        info!("(No Vendor Data)\n");
    } else {
        info!("== Vendor Data ==");
        for vendor in &model.vendors {
            info!("{vendor}");
        }
    }

    info!("Initializing Inox2D renderer...");
    let window_size = window.inner_size();
    let mut renderer = OpenglRenderer::new(gl)?;
    renderer.prepare(&model)?;
    renderer.resize(window_size.width, window_size.height);

    info!("Uploading model textures...");
    //renderer.upload_model_textures(&model.textures)?;
    renderer.camera.scale = Vec2::splat(0.15);
    info!("Inox2D renderer initialized");

    let scene_ctrl = WasmSceneController::new(&renderer.camera, 0.5);

    // Refcells because we need to make our own continuous animation loop.
    // Winit won't help us :(
    let scene_ctrl = Rc::new(RefCell::new(scene_ctrl));
    let renderer = Rc::new(RefCell::new(renderer));
    let puppet = Rc::new(RefCell::new(model.puppet));

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
                let _t = scene_ctrl.borrow().current_elapsed();
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

async fn runwrap(model_channel: &mut UnboundedReceiver<Model>) {
    match run(model_channel).await {
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

fn request_animation_frame(f: &wasm_bindgen::prelude::Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("Couldn't register `requestAnimationFrame`");
}

async fn wait_for_model(receiver: &mut UnboundedReceiver<Model>) -> anyhow::Result<Model> {
    loop {
        match receiver.try_next() {
            Ok(Some(model)) => {
                return Ok(model);
            }
            Ok(None) => {
                return Err(anyhow!("Couldn't receive model"));
            }
            Err(_) => {
                let delay = wasm_timer::Delay::new(web_time::Duration::from_millis(100));
                delay.await?; // Introduce a delay before the next attempt
            }
        }
    }
}
