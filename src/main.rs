use dioxus::prelude::*;
use dioxus_html_macro::html;
mod inox2d_component;
mod scene;
use crate::inox2d_component::inox2d_component;

fn request_animation_frame(f: &wasm_bindgen::prelude::Closure<dyn FnMut()>) {
    use wasm_bindgen::JsCast;
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("Couldn't register `requestAnimationFrame`");
}

pub fn base_url() -> String {
    web_sys::window().unwrap().location().origin().unwrap()
}

fn main() {
    // launch the web app
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    dioxus_web::launch(app);
}

// create a component that renders a div with the text "Hello, world!"
fn app(cx: Scope) -> Element {
    cx.render(html!(
        <inox2d_component href="{base_url()}/assets/puppet.inp" width={360} height={720}></inox2d_component>
    ))
}
