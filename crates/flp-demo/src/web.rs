//! Browser glue: start of the app on the canvas, file pick and download.

use std::cell::RefCell;

use eframe::wasm_bindgen::{self, prelude::*, JsCast};
use flp_core::Project;

thread_local! {
    static LOADED: RefCell<Option<Project>> = const { RefCell::new(None) };
}

/// A project chosen by the user, once its file has been read.
pub fn take_loaded() -> Option<Project> {
    LOADED.with(|l| l.borrow_mut().take())
}

/// Opens the file dialog; the picked `.flpn` lands in [`take_loaded`].
pub fn pick_project() {
    let doc = web_sys::window()
        .and_then(|w| w.document())
        .expect("document");
    let input: web_sys::HtmlInputElement = doc.create_element("input").unwrap().unchecked_into();
    input.set_type("file");
    input.set_accept(".flpn,.crab,.zip");
    let handler = Closure::once_into_js({
        let input = input.clone();
        move || {
            let Some(file) = input.files().and_then(|f| f.get(0)) else {
                return;
            };
            let name = file.name();
            let promise = file.array_buffer();
            wasm_bindgen_futures::spawn_local(async move {
                let Ok(buf) = wasm_bindgen_futures::JsFuture::from(promise).await else {
                    return;
                };
                let bytes = js_sys::Uint8Array::new(&buf).to_vec();
                if let Ok(p) = Project::from_archive_bytes(&bytes, &name) {
                    LOADED.with(|l| *l.borrow_mut() = Some(p));
                }
            });
        }
    });
    input.set_onchange(Some(handler.unchecked_ref()));
    input.click();
}

/// Saves `text` as a file through the browser.
pub fn download(name: &str, text: &str) {
    let parts = js_sys::Array::of1(&JsValue::from_str(text));
    let Ok(blob) = web_sys::Blob::new_with_str_sequence(&parts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let doc = web_sys::window()
        .and_then(|w| w.document())
        .expect("document");
    let a: web_sys::HtmlAnchorElement = doc.create_element("a").unwrap().unchecked_into();
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
}

#[wasm_bindgen]
pub async fn start(canvas_id: String) -> Result<(), JsValue> {
    let canvas = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(&canvas_id))
        .ok_or("no canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|_| Ok(Box::new(crate::DemoApp::new()))),
        )
        .await
}
