//! Sycamore-owned index and controls for the browser lab.

use crate::scenarios::Scenario;
use sycamore::prelude::*;

#[component]
fn Index() -> View {
    view! {
        main(class="shell") {
            header {
                p(class="eyebrow") { "ATM0S / CLIENT-RS / BROWSER LAB" }
                h1 { "Media paths, made observable." }
                p(class="lede") { "A small non-React harness for exercising the Rust browser adapter against a standalone gateway." }
            }
            nav(class="scenario-links", aria-label="scenario examples") {
                Indexed(list=Scenario::ALL.to_vec(), view=|scenario| view! {
                    a(href=format!("?example={}", scenario.as_str())) { (scenario.as_str()) }
                })
            }
            section(class="controls", aria-label="scenario controls") {
                label { "Scenario" select(id="scenario") {
                    Indexed(list=Scenario::ALL.to_vec(), view=|scenario| view! {
                        option(value=scenario.as_str()) { (scenario.as_str()) }
                    })
                } }
                label { "Gateway" input(id="gateway", value="http://127.0.0.1:13100") }
                label { "Room" input(id="room", value="client-rs-e2e") }
                label { "Peer" input(id="peer", value="browser-a") }
                label { "Token" input(id="token", r#type="password", autocomplete="off") }
                div(class="actions") { button(id="connect") { "Connect" } button(id="disconnect", class="quiet") { "Disconnect" } }
            }
            section(class="stage") {
                div(class="media-card") { span { "LOCAL CAPTURE" } video(id="local-video", autoplay=true, muted=true, playsinline=true) {} }
                div(class="arrow") { "→" }
                div(class="media-card") { span { "REMOTE / MIXER OUTPUT" } video(id="remote-video", autoplay=true, playsinline=true) {} audio(id="remote-audio", autoplay=true) {} video(id="extra-video", autoplay=true, muted=true, playsinline=true, hidden=true) {} }
            }
            section(class="telemetry") {
                div { h2 { "Event trace" } pre(id="event-log", aria-live="polite") {} }
                div { h2 { "Assertions" } ul(id="assertions") {
                    li(data-assert="control") { "control channel: pending" }
                    li(data-assert="scenario") { "scenario behavior: pending" }
                    li(data-assert="media") { "decoded media: pending" }
                    li(data-assert="cleanup") { "cleanup: pending" }
                } }
            }
        }
    }
}

/// Mount the Rust navigation into the empty `#scenario-index` placeholder.
pub fn mount(document: &web_sys::Document) {
    if let Some(target) = document.get_element_by_id("app") {
        sycamore::render_to(Index, &target);
    }
}
