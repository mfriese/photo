//! Compact tag selection. Membership and OR semantics stay in the core; no file access.
use super::Suite;
use crate::theme;
use eframe::egui;
impl Suite {
    pub(super) fn tag_cloud(&mut self, ui: &mut egui::Ui, fallback: &[usize]) -> Vec<usize> {
        let previous = self.tag_view.filters.clone();
        self.prepare_tag_cloud();
        let Some(root) = self.organization.root() else {
            return fallback.to_vec();
        };
        let Some(catalog) = self.organization.catalog() else {
            return fallback.to_vec();
        };
        let version = self.organization.version();
        if self
            .tag_view
            .cloud_cache
            .as_ref()
            .is_none_or(|c| c.version != version || c.count != self.items.len())
        {
            let candidates = if let Some(tag) = &self.tag_view.active {
                photosuite_application::organization::visible(
                    &self.items,
                    "",
                    false,
                    root,
                    catalog,
                    tag,
                )
            } else {
                crate::application::visible(&self.items, "", self.group.as_deref(), false)
            };
            let tags = photosuite_application::organization::available_tags(
                &self.items,
                root,
                catalog,
                &candidates,
            );
            self.tag_view.cloud_cache = Some(super::tags::CloudCache {
                version,
                count: self.items.len(),
                tags,
            });
        }
        let tags = self.tag_view.cloud_cache.as_ref().unwrap().tags.clone();
        self.tag_view.filters.retain(|tag| tags.contains(tag));
        if tags.is_empty() {
            return if previous == self.tag_view.filters {
                fallback.to_vec()
            } else {
                self.visible()
            };
        }
        egui::ScrollArea::vertical().id_salt("period_tag_cloud").max_height(72.).auto_shrink([false,true]).show(ui,|ui|{
            ui.horizontal_wrapped(|ui|{
                ui.spacing_mut().item_spacing=egui::vec2(4.,3.);
                ui.spacing_mut().interact_size.y=22.;ui.spacing_mut().button_padding.y=2.;
                ui.add_sized([30.,22.],egui::Label::new(egui::RichText::new("Tags").size(12.).color(theme::MUTED)));
                if ui.add_enabled(!self.tag_view.filters.is_empty(),egui::Button::new(egui::RichText::new("Reset").size(12.)).min_size(egui::vec2(0.,22.))).clicked(){self.tag_view.filters.clear();}
                for tag in tags {
                    let selected=self.tag_view.filters.contains(&tag);
                    let response=ui.add(egui::Button::new(egui::RichText::new(format!("    {tag}")).size(12.).color(theme::TEXT)).selected(selected).wrap_mode(egui::TextWrapMode::Truncate).min_size(egui::vec2(0.,22.)).fill(if selected{theme::SELECTED}else{theme::RAISED}));
                    if selected {
                        let point=egui::pos2(response.rect.left()+7.,response.rect.center().y);
                        ui.painter().line_segment([point+egui::vec2(-2.,0.),point+egui::vec2(0.,2.)],egui::Stroke::new(1.5_f32,theme::TEXT));
                        ui.painter().line_segment([point+egui::vec2(0.,2.),point+egui::vec2(4.,-3.)],egui::Stroke::new(1.5_f32,theme::TEXT));
                    }
                    if response.on_hover_text(format!("{tag}\nNur Medien mit mindestens einem der ausgewählten Tags anzeigen. Keine Auswahl zeigt alle.")).clicked() && !self.tag_view.filters.remove(&tag){self.tag_view.filters.insert(tag);}
                }
            });
        });
        if previous == self.tag_view.filters {
            fallback.to_vec()
        } else {
            self.visible()
        }
    }
}
