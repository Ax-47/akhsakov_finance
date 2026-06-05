use dioxus::prelude::*;
use ui::{App, Echo, Hero};

#[component]
pub fn Home() -> Element {
    rsx! {
        App{
            Hero {}
            Echo {}
        }
    }
}
