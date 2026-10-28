//! The Git Panel.

use gpui::{AnyView, AppContext as _, Context, Entity, IntoElement, Render, Window};

use crate::model::Model;

struct Pending;

impl Render for Pending {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::ui::note("", "The Git Panel is being ported", cx)
    }
}

pub fn build<T>(_model: &Entity<Model>, _window: &mut Window, cx: &mut Context<T>) -> AnyView {
    cx.new(|_| Pending).into()
}
