#![forbid(unsafe_code)]

//! Browser interface for the local PDF presentation viewer.

mod protocol;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use js_sys::{Array, ArrayBuffer, Object, Reflect, Uint8Array};
use presenter_core::{Layout, MAX_FILE_BYTES, Presentation, View};
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Document, Element, Event, HtmlCanvasElement, HtmlElement, HtmlInputElement, HtmlSelectElement,
    KeyboardEvent, MessageEvent, Window,
};

const PROTOCOL: &str = "rust-pdf-presenter/v1";
const CACHE_ENTRIES: usize = 6;
const CACHE_BYTES: usize = 64 * 1024 * 1024;
const RASTER_PIXELS: f64 = 4_000_000.0;

thread_local! {
    static APP: RefCell<Option<Rc<RefCell<App>>>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct RasterKey {
    page: usize,
    notes: bool,
    notes_right: bool,
    width: u32,
    height: u32,
}

struct CacheEntry {
    key: RasterKey,
    canvas: HtmlCanvasElement,
    bytes: usize,
}

struct TargetSize {
    key: RasterKey,
    css_width: f64,
    css_height: f64,
}

enum DeferredRender {
    Notes(usize),
    Preview(usize),
    Prefetch { page: usize, view: View },
}

struct GifListener {
    image: Element,
    attachment_id: String,
    callback: Closure<dyn FnMut(Event)>,
}

struct App {
    window: Window,
    dom: Document,
    origin: String,
    audience_role: bool,
    opener: Option<Window>,
    audience: Option<Window>,
    audience_instance: Option<String>,
    instance: String,
    sent_document: Option<u32>,
    document_id: u32,
    load_generation: u32,
    pdf: Vec<u8>,
    presentation: Option<Presentation>,
    page: usize,
    layout: Layout,
    started_ms: f64,
    cache: VecDeque<CacheEntry>,
    deferred: VecDeque<DeferredRender>,
    deferred_scheduled: bool,
    render_generation: u32,
    rendered_state: Option<(u32, usize, Layout)>,
    gif_urls: HashMap<String, String>,
    gif_listeners: Vec<GifListener>,
    failed_gifs: HashSet<String>,
    resize_generation: u32,
    connected: bool,
    callbacks: Vec<Closure<dyn FnMut(Event)>>,
}

/// Initialize the interface. Only generated bindings and a minimal module bootstrap are needed.
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No browser window"))?;
    let dom = window
        .document()
        .ok_or_else(|| JsValue::from_str("No browser document"))?;
    let href = window.location().href()?;
    let url = web_sys::Url::new(&href)?;
    let audience_role = url.search_params().get("role").as_deref() == Some("audience");
    let opener_value = window.opener()?;
    // Native Window.opener returns a WindowProxy from another realm, so an
    // instanceof Window cast incorrectly rejects it. Guard the nullable API.
    let opener = (!opener_value.is_null() && !opener_value.is_undefined())
        .then(|| opener_value.unchecked_into::<Window>());
    let origin = window.location().origin()?;
    let mount = dom
        .get_element_by_id("app")
        .ok_or_else(|| JsValue::from_str("Missing app mount"))?;
    mount.set_inner_html(if audience_role {
        AUDIENCE_HTML
    } else {
        PRESENTER_HTML
    });
    dom.set_title(if audience_role {
        "Audience · Gebakuk"
    } else {
        "Gebakuk · PDF Presenter"
    });
    let app = Rc::new(RefCell::new(App {
        window,
        dom,
        origin,
        audience_role,
        opener,
        audience: None,
        audience_instance: None,
        instance: format!("{}-{}", js_sys::Date::now(), js_sys::Math::random()),
        sent_document: None,
        document_id: 0,
        load_generation: 0,
        pdf: Vec::new(),
        presentation: None,
        page: 0,
        layout: Layout::Normal,
        started_ms: js_sys::Date::now(),
        cache: VecDeque::new(),
        deferred: VecDeque::new(),
        deferred_scheduled: false,
        render_generation: 0,
        rendered_state: None,
        gif_urls: HashMap::new(),
        gif_listeners: Vec::new(),
        failed_gifs: HashSet::new(),
        resize_generation: 0,
        connected: false,
        callbacks: Vec::new(),
    }));
    APP.with(|slot| *slot.borrow_mut() = Some(app.clone()));
    attach_events(&app)?;
    app.borrow_mut().update_controls();
    if audience_role {
        let state = app.borrow();
        state.status(
            if state.opener.is_some() {
                "Waiting for the presenter…"
            } else {
                "Open the audience window using the presenter’s “Open audience” button."
            },
            false,
        );
        state.send_ready();
    }
    Ok(())
}

fn app() -> Option<Rc<RefCell<App>>> {
    APP.with(|slot| slot.borrow().clone())
}

fn attach_events(state: &Rc<RefCell<App>>) -> Result<(), JsValue> {
    let mut state = state.borrow_mut();
    if !state.audience_role {
        state.listen_element("file-input", "change", |event| {
            let Some(input) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
            else {
                return;
            };
            let Some(file) = input.files().and_then(|files| files.get(0)) else {
                return;
            };
            input.set_value("");
            let Some(state) = app() else { return };
            let generation = {
                let mut state = state.borrow_mut();
                state.load_generation = state.load_generation.wrapping_add(1);
                if file.size() > MAX_FILE_BYTES as f64 {
                    state.status("This PDF exceeds the 64 MiB file limit.", true);
                    return;
                }
                state.status("Opening PDF…", false);
                state.load_generation
            };
            spawn_local(async move {
                let result = JsFuture::from(file.array_buffer()).await;
                let mut state = state.borrow_mut();
                if state.load_generation != generation {
                    return;
                }
                match result {
                    Ok(buffer) => {
                        let bytes = Uint8Array::new(&buffer).to_vec();
                        state.load_document(bytes, None);
                    }
                    Err(_) => state.status("The browser could not read this file.", true),
                }
            });
        })?;
        state.listen_element("layout-select", "change", |_| {
            with_app(|state| {
                let value = state
                    .dom
                    .get_element_by_id("layout-select")
                    .and_then(|element| element.dyn_into::<HtmlSelectElement>().ok())
                    .map(|select| select.value())
                    .unwrap_or_default();
                let Some(layout) = protocol::layout(&value) else {
                    return;
                };
                state.layout = layout;
                state.cache.clear();
                state.send_sync("sync");
                state.render();
            });
        })?;
        state.listen_element("audience-button", "click", |_| with_app(App::open_audience))?;
        state.listen_element("previous-button", "click", |_| {
            with_app(|state| state.navigate("previous"))
        })?;
        state.listen_element("next-button", "click", |_| {
            with_app(|state| state.navigate("next"))
        })?;
        state.listen_element("reset-timer-button", "click", |_| {
            with_app(|state| {
                state.started_ms = js_sys::Date::now();
                state.update_timer();
                state.send_sync("sync");
            })
        })?;
    }
    state.listen_element("fullscreen-button", "click", |_| with_app(App::fullscreen))?;
    state.listen_element("current-slide", "dblclick", |_| with_app(App::fullscreen))?;
    state.listen_window("keydown", |event| {
        let Some(event) = event.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if event.ctrl_key() || event.meta_key() || event.alt_key() {
            return;
        }
        if let Some(target) = event
            .target()
            .and_then(|target| target.dyn_into::<Element>().ok())
            && target
                .closest("input,select,textarea,button,a[href],[contenteditable]")
                .ok()
                .flatten()
                .is_some()
        {
            return;
        }
        let action = match event.key().as_str() {
            "ArrowRight" | "PageDown" | " " => "next",
            "ArrowLeft" | "PageUp" => "previous",
            "Home" => "first",
            "End" => "last",
            _ => return,
        };
        event.prevent_default();
        with_app(|state| {
            if state.audience_role {
                let message = state.message("command");
                set(&message, "action", &JsValue::from_str(action));
                state.send_opener(&message);
            } else {
                state.navigate(action);
            }
        });
    })?;
    state.listen_window("resize", |_| schedule_resize())?;
    state.listen_document("fullscreenchange", |_| schedule_resize())?;
    state.listen_window("message", |event| {
        let Some(event) = event.dyn_ref::<MessageEvent>() else {
            return;
        };
        with_app(|state| state.receive(event));
    })?;
    state.listen_window("beforeunload", |_| {
        with_app(|state| {
            let message = state.message("disconnect");
            if state.audience_role {
                state.send_opener(&message);
            } else {
                state.send_audience(&message);
            }
            state.clear_gifs();
        })
    })?;
    let callback =
        Closure::wrap(Box::new(move |_event: Event| with_app(App::tick)) as Box<dyn FnMut(Event)>);
    state
        .window
        .set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            1000,
        )?;
    state.callbacks.push(callback);
    Ok(())
}

fn with_app(action: impl FnOnce(&mut App)) {
    if let Some(state) = app() {
        action(&mut state.borrow_mut());
    }
}

fn schedule_resize() {
    let Some(state) = app() else { return };
    let generation = {
        let mut state = state.borrow_mut();
        state.resize_generation = state.resize_generation.wrapping_add(1);
        state.invalidate_render();
        state.resize_generation
    };
    let callback = Closure::once_into_js(move || {
        with_app(|state| {
            if state.resize_generation == generation {
                state.render();
            }
        });
    });
    let _ = state
        .borrow()
        .window
        .set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 120);
}

impl App {
    fn listen_element(
        &mut self,
        id: &str,
        event: &str,
        callback: impl FnMut(Event) + 'static,
    ) -> Result<(), JsValue> {
        let target = self
            .dom
            .get_element_by_id(id)
            .ok_or_else(|| JsValue::from_str(id))?;
        let closure = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())?;
        self.callbacks.push(closure);
        Ok(())
    }

    fn listen_window(
        &mut self,
        event: &str,
        callback: impl FnMut(Event) + 'static,
    ) -> Result<(), JsValue> {
        let closure = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
        self.window
            .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())?;
        self.callbacks.push(closure);
        Ok(())
    }

    fn listen_document(
        &mut self,
        event: &str,
        callback: impl FnMut(Event) + 'static,
    ) -> Result<(), JsValue> {
        let closure = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
        self.dom
            .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())?;
        self.callbacks.push(closure);
        Ok(())
    }

    fn status(&self, text: &str, error: bool) {
        if let Some(element) = self.dom.get_element_by_id("status") {
            element.set_text_content(Some(text));
            element.set_class_name(if error { "status error" } else { "status" });
        }
    }

    fn load_document(&mut self, bytes: Vec<u8>, remote_id: Option<u32>) {
        if bytes.len() > MAX_FILE_BYTES {
            self.status("This PDF exceeds the 64 MiB file limit.", true);
            return;
        }
        match Presentation::load(bytes.clone()) {
            Ok(presentation) => {
                self.clear_gifs();
                self.failed_gifs.clear();
                self.cache.clear();
                self.invalidate_render();
                self.pdf = bytes;
                self.presentation = Some(presentation);
                self.document_id = remote_id.unwrap_or_else(|| self.document_id.wrapping_add(1));
                self.page = 0;
                self.started_ms = js_sys::Date::now();
                self.sent_document = None;
                self.connected = true;
                let diagnostics = self
                    .presentation
                    .as_ref()
                    .map(|pdf| pdf.diagnostics().join(" · "))
                    .unwrap_or_default();
                self.status(
                    if diagnostics.is_empty() {
                        "PDF ready."
                    } else {
                        &diagnostics
                    },
                    false,
                );
                self.update_controls();
                if !self.audience_role {
                    self.transfer_document();
                    self.render();
                }
            }
            Err(error) => {
                let message = format!("Unable to open PDF: {error}");
                self.status(&message, true);
                if self.audience_role {
                    let event = self.message("error");
                    set(&event, "message", &JsValue::from_str(&message));
                    self.send_opener(&event);
                }
            }
        }
    }

    fn navigate(&mut self, action: &str) {
        let Some(pdf) = self.presentation.as_ref() else {
            return;
        };
        let Some(next) = protocol::navigation_target(self.page, pdf.page_count(), action) else {
            return;
        };
        if next == self.page {
            return;
        }
        self.page = next;
        self.send_sync("navigate");
        self.render();
    }

    fn render(&mut self) {
        self.invalidate_render();
        self.clear_gifs();
        if self.presentation.is_none() {
            return;
        }
        self.update_controls();
        match self.render_target("current-slide", self.page, View::Audience, true, None) {
            Ok(()) => self.rendered_state = Some((self.document_id, self.page, self.layout)),
            Err(error) => self.render_error("current-slide", error),
        }
        if !self.audience_role {
            if matches!(self.layout, Layout::NotesRight) {
                let cached = self
                    .target_size("notes-slide", self.page, View::Notes)
                    .is_ok_and(|size| self.cache.iter().any(|entry| entry.key == size.key));
                if cached {
                    if let Err(error) =
                        self.render_target("notes-slide", self.page, View::Notes, true, None)
                    {
                        self.render_error("notes-slide", error);
                    }
                } else {
                    self.clear_view("notes-slide");
                    self.deferred.push_back(DeferredRender::Notes(self.page));
                }
            } else if let Some(notes) = self.dom.get_element_by_id("notes-slide") {
                notes.set_inner_html(
                    "<p class=\"empty-hint\">Choose “Notes on right” for double-width PDFs.</p>",
                );
            }
            if self.page + 1
                < self
                    .presentation
                    .as_ref()
                    .map_or(0, Presentation::page_count)
            {
                self.clear_view("next-slide");
                self.deferred
                    .push_back(DeferredRender::Preview(self.page + 1));
            } else if let Some(next) = self.dom.get_element_by_id("next-slide") {
                next.set_inner_html("<p class=\"empty-hint\">End of presentation</p>");
            }
        }
        let count = self
            .presentation
            .as_ref()
            .map_or(0, Presentation::page_count);
        if self.page + 1 < count {
            if self.audience_role {
                self.deferred.push_back(DeferredRender::Prefetch {
                    page: self.page + 1,
                    view: View::Audience,
                });
            } else if matches!(self.layout, Layout::NotesRight) {
                self.deferred.push_back(DeferredRender::Prefetch {
                    page: self.page + 1,
                    view: View::Notes,
                });
            }
        }
        if self.page > 0 {
            self.deferred.push_back(DeferredRender::Prefetch {
                page: self.page - 1,
                view: View::Audience,
            });
            if !self.audience_role && matches!(self.layout, Layout::NotesRight) {
                self.deferred.push_back(DeferredRender::Prefetch {
                    page: self.page - 1,
                    view: View::Notes,
                });
            }
        }
        self.schedule_deferred();
    }

    fn invalidate_render(&mut self) {
        self.render_generation = self.render_generation.wrapping_add(1);
        self.rendered_state = None;
        self.deferred.clear();
    }

    fn clear_view(&self, id: &str) {
        if let Some(host) = self.dom.get_element_by_id(id) {
            host.set_inner_html("");
        }
    }

    fn render_error(&self, id: &str, error: String) {
        if let Some(host) = self.dom.get_element_by_id(id) {
            host.set_inner_html("<p class=\"empty-hint\">This page could not be rendered.</p>");
        }
        let error = format!("Unable to render this page: {error}");
        self.status(&error, true);
        if self.audience_role {
            let message = self.message("error");
            set(&message, "message", &JsValue::from_str(&error));
            self.send_opener(&message);
        }
    }

    fn schedule_deferred(&mut self) {
        if self.deferred_scheduled || self.deferred.is_empty() {
            return;
        }
        self.deferred_scheduled = true;
        // A RAF callback runs before paint. Yield once more so rasterizing a
        // preview or neighbor cannot delay painting the selected slide.
        let callback = Closure::once_into_js(move |_timestamp: f64| {
            with_app(|state| {
                let generation = state.render_generation;
                let callback = Closure::once_into_js(move || {
                    with_app(|state| {
                        state.deferred_scheduled = false;
                        if state.render_generation == generation
                            && let Some(task) = state.deferred.pop_front()
                        {
                            state.run_deferred(task);
                        }
                        state.schedule_deferred();
                    });
                });
                if state
                    .window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        callback.unchecked_ref(),
                        0,
                    )
                    .is_err()
                {
                    state.deferred_scheduled = false;
                }
            });
        });
        if self
            .window
            .request_animation_frame(callback.unchecked_ref())
            .is_err()
        {
            self.deferred_scheduled = false;
        }
    }

    fn run_deferred(&mut self, task: DeferredRender) {
        match task {
            DeferredRender::Notes(page) => {
                if let Err(error) = self.render_target("notes-slide", page, View::Notes, true, None)
                {
                    self.render_error("notes-slide", error);
                }
            }
            DeferredRender::Preview(page) => {
                // Render the preview at the size needed when this becomes the
                // current slide, then reuse that same canvas on navigation.
                let result = self
                    .target_size("current-slide", page, View::Audience)
                    .and_then(|size| {
                        self.render_target(
                            "next-slide",
                            page,
                            View::Audience,
                            false,
                            Some(size.key),
                        )
                    });
                if let Err(error) = result {
                    self.render_error("next-slide", error);
                }
            }
            DeferredRender::Prefetch { page, view } => {
                let id = if matches!(view, View::Notes) {
                    "notes-slide"
                } else {
                    "current-slide"
                };
                if let Ok(size) = self.target_size(id, page, view) {
                    let _ = self.cached_canvas(size.key);
                }
            }
        }
    }

    fn target_size(&self, id: &str, page: usize, view: View) -> Result<TargetSize, String> {
        let host = self
            .dom
            .get_element_by_id(id)
            .ok_or("Missing slide viewport")?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "Invalid slide viewport")?;
        let pdf = self.presentation.as_ref().ok_or("No PDF")?;
        let region = pdf
            .region(page, self.layout, view)
            .map_err(|error| error.to_string())?;
        let available_width = f64::from(host.client_width()).max(1.0);
        let available_height = f64::from(host.client_height()).max(1.0);
        let scale = (available_width / region.width).min(available_height / region.height);
        let css_width = (region.width * scale).max(1.0);
        let css_height = (region.height * scale).max(1.0);
        let dpr = self.window.device_pixel_ratio().clamp(1.0, 2.0);
        let mut pixel_width = (css_width * dpr).round().max(1.0);
        let mut pixel_height = (css_height * dpr).round().max(1.0);
        let pixel_scale = (RASTER_PIXELS / (pixel_width * pixel_height))
            .sqrt()
            .min(1.0)
            .min(4096.0 / pixel_width)
            .min(4096.0 / pixel_height);
        pixel_width = (pixel_width * pixel_scale).floor().max(1.0);
        pixel_height = (pixel_height * pixel_scale).floor().max(1.0);
        let key = RasterKey {
            page,
            notes: matches!(view, View::Notes),
            notes_right: matches!(self.layout, Layout::NotesRight),
            width: pixel_width as u32,
            height: pixel_height as u32,
        };
        Ok(TargetSize {
            key,
            css_width,
            css_height,
        })
    }

    fn cached_canvas(&mut self, key: RasterKey) -> Result<HtmlCanvasElement, String> {
        if let Some(index) = self.cache.iter().position(|entry| entry.key == key) {
            let entry = self.cache.remove(index).ok_or("Render cache error")?;
            let canvas = entry.canvas.clone();
            self.cache.push_front(entry);
            return Ok(canvas);
        }
        let view = if key.notes {
            View::Notes
        } else {
            View::Audience
        };
        let layout = if key.notes_right {
            Layout::NotesRight
        } else {
            Layout::Normal
        };
        let raster = self
            .presentation
            .as_ref()
            .ok_or("No PDF")?
            .render(key.page, layout, view, key.width, key.height)
            .map_err(|error| error.to_string())?;
        let canvas = self
            .dom
            .create_element("canvas")
            .map_err(js_error)?
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| "Invalid canvas")?;
        canvas.set_width(raster.width);
        canvas.set_height(raster.height);
        canvas
            .set_attribute("aria-label", &format!("Slide {}", key.page + 1))
            .map_err(js_error)?;
        let context = canvas
            .get_context("2d")
            .map_err(js_error)?
            .ok_or("Canvas is unavailable")?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| "Invalid canvas context")?;
        let pixels = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&raster.rgba),
            raster.width,
            raster.height,
        )
        .map_err(js_error)?;
        context
            .put_image_data(&pixels, 0.0, 0.0)
            .map_err(js_error)?;
        self.cache.push_front(CacheEntry {
            key,
            canvas: canvas.clone(),
            bytes: raster.rgba.len(),
        });
        while self.cache.len() > CACHE_ENTRIES
            || self.cache.iter().map(|entry| entry.bytes).sum::<usize>() > CACHE_BYTES
        {
            self.cache.pop_back();
        }
        Ok(canvas)
    }

    fn render_target(
        &mut self,
        id: &str,
        page: usize,
        view: View,
        animated: bool,
        render_key: Option<RasterKey>,
    ) -> Result<(), String> {
        let size = self.target_size(id, page, view)?;
        let canvas = self.cached_canvas(render_key.unwrap_or(size.key))?;
        let host = self
            .dom
            .get_element_by_id(id)
            .ok_or("Missing slide viewport")?;
        host.set_inner_html("");
        let surface = self
            .dom
            .create_element("div")
            .map_err(js_error)?
            .dyn_into::<HtmlElement>()
            .map_err(|_| "Invalid surface")?;
        surface.set_class_name("slide-surface");
        surface
            .style()
            .set_property("width", &format!("{}px", size.css_width))
            .map_err(js_error)?;
        surface
            .style()
            .set_property("height", &format!("{}px", size.css_height))
            .map_err(js_error)?;
        surface.append_child(&canvas).map_err(js_error)?;
        if animated {
            let pdf = self.presentation.as_ref().ok_or("No PDF")?;
            for overlay in pdf.animations(page, self.layout, view) {
                if self.failed_gifs.contains(&overlay.attachment_id) {
                    continue;
                }
                let Some(gif) = pdf.attachment(&overlay.attachment_id) else {
                    continue;
                };
                let url = if let Some(url) = self.gif_urls.get(&overlay.attachment_id) {
                    url.clone()
                } else {
                    let bytes = Uint8Array::from(gif.bytes.as_slice());
                    let parts = Array::new();
                    parts.push(&bytes.buffer());
                    let options = web_sys::BlobPropertyBag::new();
                    options.set_type("image/gif");
                    let blob =
                        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
                            .map_err(js_error)?;
                    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;
                    self.gif_urls
                        .insert(overlay.attachment_id.clone(), url.clone());
                    url
                };
                let image = self.dom.create_element("img").map_err(js_error)?;
                image.set_class_name("gif-overlay");
                let attachment_id = overlay.attachment_id.clone();
                let error_url = url.clone();
                let error_id = attachment_id.clone();
                let callback = Closure::wrap(Box::new(move |_event: Event| {
                    with_app(|state| state.gif_failed(&error_id, &error_url));
                }) as Box<dyn FnMut(Event)>);
                image
                    .add_event_listener_with_callback("error", callback.as_ref().unchecked_ref())
                    .map_err(js_error)?;
                self.gif_listeners.push(GifListener {
                    image: image.clone(),
                    attachment_id,
                    callback,
                });
                image.set_attribute("src", &url).map_err(js_error)?;
                image
                    .set_attribute("alt", "Animated slide illustration")
                    .map_err(js_error)?;
                let rect = overlay.rect;
                image
                    .set_attribute(
                        "style",
                        &format!(
                            "left:{}%;top:{}%;width:{}%;height:{}%",
                            rect.x * 100.0,
                            rect.y * 100.0,
                            rect.width * 100.0,
                            rect.height * 100.0
                        ),
                    )
                    .map_err(js_error)?;
                surface.append_child(&image).map_err(js_error)?;
            }
        }
        host.append_child(&surface).map_err(js_error)?;
        Ok(())
    }

    fn clear_gifs(&mut self) {
        // Detach before dropping callbacks, including callbacks that have fired.
        for listener in self.gif_listeners.drain(..) {
            let _ = listener.image.remove_event_listener_with_callback(
                "error",
                listener.callback.as_ref().unchecked_ref(),
            );
            listener.image.remove();
        }
        for (_, url) in self.gif_urls.drain() {
            let _ = web_sys::Url::revoke_object_url(&url);
        }
    }

    fn gif_failed(&mut self, attachment_id: &str, url: &str) {
        // Ignore a queued error from an earlier render. Keep callback storage
        // until the next redraw so this function cannot drop its own closure.
        if self
            .gif_urls
            .get(attachment_id)
            .is_none_or(|active| active != url)
        {
            return;
        }
        self.failed_gifs.insert(attachment_id.to_owned());
        for listener in &self.gif_listeners {
            if listener.attachment_id == attachment_id {
                listener.image.remove();
            }
        }
        if let Some(url) = self.gif_urls.remove(attachment_id) {
            let _ = web_sys::Url::revoke_object_url(&url);
        }
        let error = "An animation could not be decoded. Its static PDF image is shown instead.";
        self.status(error, true);
        if self.audience_role {
            let message = self.message("error");
            set(&message, "message", &JsValue::from_str(error));
            self.send_opener(&message);
        }
    }

    fn update_controls(&self) {
        let count = self
            .presentation
            .as_ref()
            .map_or(0, Presentation::page_count);
        if let Some(label) = self.dom.get_element_by_id("slide-count") {
            label.set_text_content(Some(&if count == 0 {
                "— / —".into()
            } else {
                format!("{} / {count}", self.page + 1)
            }));
        }
        for (id, disabled) in [
            ("previous-button", self.page == 0 || count == 0),
            ("next-button", count == 0 || self.page + 1 >= count),
        ] {
            if let Some(button) = self.dom.get_element_by_id(id) {
                if disabled {
                    let _ = button.set_attribute("disabled", "");
                } else {
                    let _ = button.remove_attribute("disabled");
                }
            }
        }
        self.update_timer();
    }

    fn update_timer(&self) {
        if let Some(timer) = self.dom.get_element_by_id("elapsed") {
            let seconds = if self.presentation.is_some() {
                ((js_sys::Date::now() - self.started_ms).max(0.0) / 1000.0) as u64
            } else {
                0
            };
            timer.set_text_content(Some(&format!(
                "{:02}:{:02}:{:02}",
                seconds / 3600,
                (seconds / 60) % 60,
                seconds % 60
            )));
        }
    }

    fn tick(&mut self) {
        self.update_timer();
        if self.audience_role {
            if self.connected
                && self
                    .opener
                    .as_ref()
                    .is_none_or(|window| window.closed().unwrap_or(true))
            {
                self.connected = false;
                self.status(
                    "Presenter disconnected. This is the last received slide.",
                    true,
                );
            }
        } else if self
            .audience
            .as_ref()
            .is_some_and(|window| window.closed().unwrap_or(true))
        {
            self.audience = None;
            self.sent_document = None;
            self.audience_instance = None;
            self.status("Audience window closed. Open it again to resume.", false);
        }
    }

    fn fullscreen(&mut self) {
        let target = if self.audience_role {
            self.dom.document_element()
        } else {
            self.dom.get_element_by_id("current-slide")
        };
        if let Some(target) = target {
            // web-sys currently discards requestFullscreen's Promise; retain it to
            // report browser rejections such as missing user activation.
            let request = Reflect::get(&target, &JsValue::from_str("requestFullscreen"))
                .and_then(|function| function.dyn_into::<js_sys::Function>())
                .and_then(|function| function.call0(&target));
            match request {
                Ok(result) => spawn_local(async move {
                    if let Err(error) = JsFuture::from(js_sys::Promise::resolve(&result)).await {
                        with_app(|state| {
                            state.status(
                                &format!("Fullscreen is unavailable: {}", js_error(error)),
                                true,
                            )
                        });
                    }
                }),
                Err(error) => self.status(
                    &format!("Fullscreen is unavailable: {}", js_error(error)),
                    true,
                ),
            }
        }
    }

    fn open_audience(&mut self) {
        if let Some(window) = self
            .audience
            .as_ref()
            .filter(|window| !window.closed().unwrap_or(true))
        {
            let _ = window.focus();
            self.send_sync("sync");
            return;
        }
        let result = self
            .window
            .location()
            .href()
            .and_then(|href| web_sys::Url::new(&href));
        let Ok(url) = result else {
            self.status("Unable to create audience URL.", true);
            return;
        };
        url.search_params().set("role", "audience");
        match self
            .window
            .open_with_url_and_target(&url.href(), "rust-pdf-presenter-audience")
        {
            Ok(Some(window)) => {
                self.audience = Some(window);
                self.sent_document = None;
                self.audience_instance = None;
                self.status("Audience window opened.", false);
            }
            _ => self.status(
                "The browser blocked the audience window. Allow popups and try again.",
                true,
            ),
        }
    }

    fn message(&self, kind: &str) -> Object {
        let message = Object::new();
        set(&message, "protocol", &JsValue::from_str(PROTOCOL));
        set(&message, "kind", &JsValue::from_str(kind));
        set(
            &message,
            "document",
            &JsValue::from_f64(f64::from(self.document_id)),
        );
        message
    }

    fn send_opener(&self, message: &Object) {
        if let Some(window) = &self.opener {
            let _ = window.post_message(message, &self.origin);
        }
    }

    fn send_audience(&self, message: &Object) {
        if let Some(window) = &self.audience {
            let _ = window.post_message(message, &self.origin);
        }
    }

    fn send_ready(&self) {
        let message = self.message("ready");
        set(&message, "instance", &JsValue::from_str(&self.instance));
        self.send_opener(&message);
    }

    fn fill_sync(&self, message: &Object) {
        set(message, "page", &JsValue::from_f64(self.page as f64));
        set(
            message,
            "layout",
            &JsValue::from_str(if matches!(self.layout, Layout::NotesRight) {
                "notes-right"
            } else {
                "normal"
            }),
        );
        set(
            message,
            "elapsed",
            &JsValue::from_f64((js_sys::Date::now() - self.started_ms).max(0.0)),
        );
    }

    fn send_sync(&self, kind: &str) {
        if self.audience_role {
            return;
        }
        let message = self.message(kind);
        self.fill_sync(&message);
        self.send_audience(&message);
    }

    fn transfer_document(&mut self) {
        let Some(window) = &self.audience else { return };
        if self.audience_instance.is_none() || self.presentation.is_none() {
            return;
        }
        if self.sent_document == Some(self.document_id) {
            self.send_sync("sync");
            return;
        }
        let message = self.message("document");
        self.fill_sync(&message);
        let bytes = Uint8Array::from(self.pdf.as_slice());
        let buffer = bytes.buffer();
        set(&message, "bytes", &buffer);
        let transfer = Array::new();
        transfer.push(&buffer);
        let reset = self.message("reset");
        self.send_audience(&reset);
        match window.post_message_with_transfer(&message, &self.origin, &transfer) {
            Ok(()) => self.sent_document = Some(self.document_id),
            Err(_) => self.status(
                "The PDF could not be transferred to the audience window.",
                true,
            ),
        }
    }

    fn receive(&mut self, event: &MessageEvent) {
        if event.origin() != self.origin {
            return;
        }
        let expected = if self.audience_role {
            self.opener.as_ref()
        } else {
            self.audience.as_ref()
        };
        let Some(expected) = expected else { return };
        let Some(source) = event.source() else { return };
        if !Object::is(source.as_ref(), expected.as_ref()) {
            return;
        }
        let data = event.data();
        if string(&data, "protocol").as_deref() != Some(PROTOCOL) {
            return;
        }
        let Some(kind) = string(&data, "kind") else {
            return;
        };
        if !self.audience_role {
            match kind.as_str() {
                "ready" => {
                    let Some(instance) =
                        string(&data, "instance").filter(|value| value.len() <= 100)
                    else {
                        return;
                    };
                    if self.audience_instance.as_ref() != Some(&instance) {
                        self.sent_document = None;
                        self.audience_instance = Some(instance);
                    }
                    self.transfer_document();
                }
                "command" => {
                    if integer(&data, "document", u32::MAX) == Some(self.document_id)
                        && let Some(action) = string(&data, "action")
                    {
                        self.navigate(&action);
                    }
                }
                "disconnect" => {
                    self.status("Audience disconnected. Open it again to resume.", false)
                }
                "error" => {
                    if let Some(error) = string(&data, "message") {
                        self.status(
                            &format!("Audience: {}", error.chars().take(500).collect::<String>()),
                            true,
                        );
                    }
                }
                _ => {}
            }
            return;
        }
        match kind.as_str() {
            "document" => {
                let Some(id) = integer(&data, "document", u32::MAX) else {
                    return;
                };
                let Ok(buffer) = Reflect::get(&data, &JsValue::from_str("bytes")) else {
                    return;
                };
                let Ok(buffer) = buffer.dyn_into::<ArrayBuffer>() else {
                    return;
                };
                if buffer.byte_length() as usize > MAX_FILE_BYTES {
                    self.status("Incoming PDF exceeds the file limit.", true);
                    return;
                }
                self.load_document(Uint8Array::new(&buffer).to_vec(), Some(id));
                self.apply_sync(&data);
            }
            "sync" | "navigate" => {
                if integer(&data, "document", u32::MAX) == Some(self.document_id) {
                    self.apply_sync(&data);
                }
            }
            "reset" => {
                self.invalidate_render();
                self.clear_gifs();
                self.failed_gifs.clear();
                self.cache.clear();
                self.presentation = None;
                self.pdf.clear();
                self.status("Opening presentation…", false);
            }
            "disconnect" => {
                self.connected = false;
                self.status(
                    "Presenter disconnected. This is the last received slide.",
                    true,
                );
            }
            _ => {}
        }
    }

    fn apply_sync(&mut self, data: &JsValue) {
        let Some(pdf) = self.presentation.as_ref() else {
            return;
        };
        let Some(page) =
            number(data, "page").and_then(|value| protocol::page_index(value, pdf.page_count()))
        else {
            return;
        };
        let Some(layout) = string(data, "layout").and_then(|value| protocol::layout(&value)) else {
            return;
        };
        self.page = page;
        self.layout = layout;
        if let Some(elapsed) = number(data, "elapsed")
            .filter(|value| *value >= 0.0 && *value <= 365.0 * 86400.0 * 1000.0)
        {
            self.started_ms = js_sys::Date::now() - elapsed;
        }
        self.connected = true;
        self.status("", false);
        if self.rendered_state != Some((self.document_id, self.page, self.layout)) {
            self.render();
        } else {
            self.update_controls();
        }
    }
}

fn set(object: &Object, name: &str, value: &JsValue) {
    let _ = Reflect::set(object, &JsValue::from_str(name), value);
}

fn string(object: &JsValue, name: &str) -> Option<String> {
    Reflect::get(object, &JsValue::from_str(name))
        .ok()?
        .as_string()
}

fn number(object: &JsValue, name: &str) -> Option<f64> {
    Reflect::get(object, &JsValue::from_str(name))
        .ok()?
        .as_f64()
        .filter(|number| number.is_finite())
}

fn integer(object: &JsValue, name: &str, maximum: u32) -> Option<u32> {
    number(object, name).and_then(|value| protocol::bounded_integer(value, maximum))
}

fn js_error(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "Browser API error".into())
}

const PRESENTER_HTML: &str = r#"
<div class="presenter-shell">
  <header class="toolbar">
    <div class="brand"><span class="brand-mark" aria-hidden="true">▣</span><h1>Gebakuk</h1></div>
    <label class="file-button">Open PDF<input id="file-input" type="file" accept="application/pdf,.pdf"></label>
    <label class="layout-control">Page layout<select id="layout-select"><option value="normal">Normal slides</option><option value="notes-right">Notes on right</option></select></label>
    <button id="audience-button" class="primary-button" type="button">Open audience ↗</button>
  </header>
  <div id="status" class="status" role="status" aria-live="polite"></div>
  <div class="presenter-grid">
    <section class="viewer-panel current-panel"><div class="panel-heading"><h2>Current slide</h2><span id="slide-count">— / —</span></div><div id="current-slide" class="slide-viewport" tabindex="0"><div class="empty-hint empty-state"><span class="empty-icon" aria-hidden="true">▧</span><p>Open a PDF</p><a href="./sample.pdf" download>Download the sample PDF</a></div></div></section>
    <section class="viewer-panel notes-panel"><div class="panel-heading"><h2>Speaker notes</h2><span>Presenter only</span></div><div id="notes-slide" class="slide-viewport"><p class="empty-hint">Choose “Notes on right” for double-width PDFs.</p></div></section>
    <section class="viewer-panel next-panel"><div class="panel-heading"><h2>Next slide</h2></div><div id="next-slide" class="slide-viewport"></div></section>
  </div>
  <footer class="presentation-controls"><div class="navigation"><button id="previous-button" type="button" aria-label="Previous slide">← Previous</button><button id="next-button" type="button">Next →</button><span class="keyboard-hint">← → or Space</span></div><div class="timer"><span>Elapsed</span><output id="elapsed">00:00:00</output><button id="reset-timer-button" type="button" aria-label="Reset elapsed timer">Reset</button></div><button id="fullscreen-button" type="button">Fullscreen ⛶</button></footer>
</div>"#;

const AUDIENCE_HTML: &str = r#"
<div class="audience-shell"><div id="current-slide" class="slide-viewport" tabindex="0"><p class="empty-hint">Waiting for the presenter…</p></div><div id="status" class="status" role="status" aria-live="polite"></div><button id="fullscreen-button" class="audience-fullscreen" type="button">Fullscreen ⛶</button></div>"#;
