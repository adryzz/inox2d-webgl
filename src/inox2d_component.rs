use dioxus::prelude::use_future;
use dioxus_html_macro::html;
use dioxus::prelude::*;

#[inline_props]
pub fn inox2d_component<'a>(cx: Scope, href: &'a str) -> Element<'a> {
    let uri = href.to_string();
    let puppet = use_future(cx, (), |()| async move {
        return reqwest::get(uri)
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
    });

    cx.render(html!(
        
    ))
}