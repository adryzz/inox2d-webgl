use dioxus::prelude::*;
use dioxus_html_macro::html;
mod inox2d_component;
mod scene;
use crate::inox2d_component::inox2d_component;

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
        <inox2d_component href="https://raw.githubusercontent.com/RavioliMavioli/archlive2d/main/Inochi2D/Arch%20Chan%20Model.inp" width={360} height={720}></inox2d_component>
    ))
}
