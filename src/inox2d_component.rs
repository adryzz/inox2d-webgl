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

#[inline_props]
pub fn inox2d_component<'a>(cx: Scope, href: &'a str, width: u32, height: u32) -> Element<'a> {
    let uri = href.to_string();
    let puppet = use_future(cx, (), |()| async move {
        return reqwest::get(uri).await.unwrap().bytes().await.unwrap();
    });
    cx.render(html!(
        <canvas id="canvas" width="{width}" height="{height}" tabindex="0"></canvas>
    ))
}

async fn run() -> anyhow::Result<()> {
    let canvas = get_canvas_element().ok_or(anyhow!("Couldn't find canvas"))?;

    let canvas_size = (canvas.width(), canvas.height());

    let events = winit::event_loop::EventLoop::new();
    let window = WindowBuilder::new()
        .with_canvas(Some(canvas.clone()))
        .with_resizable(false)
        .with_inner_size(PhysicalSize::new(canvas_size.0, canvas_size.1));

    let wgl2 = setup_wgl2(canvas)?;

    info!("Loading puppet");

    Ok(())
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
