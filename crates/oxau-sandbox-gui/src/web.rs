//! Web entrypoint for the Oxau sandbox GUI.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::{HtmlCanvasElement, window};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
/// Launch the GUI inside the checked-in web canvas.
pub async fn run() {
    console_error_panic_hook::set_once();

    let document = window()
        .and_then(|window| window.document())
        .expect("document not available");
    let canvas = document
        .get_element_by_id("oxau-sandbox")
        .expect("canvas with id 'oxau-sandbox' not found")
        .dyn_into::<HtmlCanvasElement>()
        .expect("element is not a canvas");

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| Ok(Box::new(oxau_sandbox_gui::SandboxApp::new(cc)))),
        )
        .await
        .expect("failed to start eframe web app");
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}

#[cfg(target_arch = "wasm32")]
fn main() {}
