//! The center tabs.

use std::sync::Arc;

use gpui::{Context, Entity, Window};
use gpui_component::dock::BasePanelView;

use crate::model::Model;

pub enum Kind {
    Diff,
    Journal,
    Commit,
}

pub fn build<T>(
    _model: &Entity<Model>,
    _window: &mut Window,
    _cx: &mut Context<T>,
) -> Vec<(Kind, Arc<dyn BasePanelView>)> {
    Vec::new()
}
