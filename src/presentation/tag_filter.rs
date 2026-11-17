//! Sidebar tag projection; membership and intersection remain in the core.
use super::Suite;
impl Suite {
    pub(super) fn prepare_tag_filter(&mut self) {
        let scope = (self.root.clone(), self.group.clone());
        if self.tag_view.filter_scope.as_ref() != Some(&scope) {
            self.tag_view.filters.clear();
            self.tag_view.options_cache = None;
            self.tag_view.filter_scope = Some(scope);
        }
        let (Some(root), Some(catalog)) = (self.organization.root(), self.organization.catalog())
        else {
            return;
        };
        let version = self.organization.version();
        if self
            .tag_view
            .options_cache
            .as_ref()
            .is_none_or(|c| c.version != version || c.count != self.items.len())
        {
            let candidates =
                crate::application::visible(&self.items, "", self.group.as_deref(), false);
            let tags = photosuite_application::organization::available_tags(
                &self.items,
                root,
                catalog,
                &candidates,
            );
            self.tag_view.options_cache = Some(super::tags::TagOptionsCache {
                version,
                count: self.items.len(),
                tags,
            });
        }
        // Do not discard selections during incremental loading: only removed definitions expire.
        self.tag_view
            .filters
            .retain(|tag| catalog.tags().contains_key(tag));
    }
}
