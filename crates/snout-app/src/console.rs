//! The Claude Code console in the bottom dock.

use std::sync::Arc;

use gpui::{Context, Entity, Window};
use gpui_component::dock::BasePanelView;

use crate::model::Model;

pub fn build<T>(_model: &Entity<Model>, _window: &mut Window, _cx: &mut Context<T>) -> Option<Arc<dyn BasePanelView>> {
    None
}
