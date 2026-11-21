use super::preview_uri;
use crate::theme;
use eframe::egui;
fn recent_libraries(
    ui: &mut egui::Ui,
    preferences: &photosuite_application::preferences::Preferences,
) -> Option<std::path::PathBuf> {
    if preferences.recent_libraries.is_empty() {
        return None;
    }
    ui.add_space(8.);
    ui.label(
        egui::RichText::new("Zuletzt geöffnete Bibliotheken")
            .small()
            .color(theme::MUTED),
    );
    let mut selected = None;
    let width = ui.available_width().min(460.);
    for path in &preferences.recent_libraries {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy();
        let parent = path.parent().unwrap_or(std::path::Path::new(""));
        let label = format!("{name}  ·  {}", parent.display());
        if ui
            .add_sized(
                [width, 26.],
                egui::Button::new(label).wrap_mode(egui::TextWrapMode::Truncate),
            )
            .on_hover_text(path.display().to_string())
            .clicked()
        {
            selected = Some(path.clone());
        }
    }
    selected
}
impl super::Suite {
    pub(super) fn photo_wall(&mut self, ctx: &egui::Context, visible: &[usize]) {
        if crate::domain::diary::audio_year(self.group.as_deref().unwrap_or("")).is_some() {
            self.diary_wall(ctx, visible);
            return;
        }
        let mut open_media = None;
        let mut recent_selection = None;
        egui::CentralPanel::default()
            .frame(theme::panel(theme::CANVAS, 12))
            .show(ctx, |ui| {
                if self.root.is_none() {
                    ui.vertical_centered(|ui| {
                        let compact = !self.preferences.recent_libraries.is_empty() && ui.available_height() < 520.;
                        ui.add_space(if compact {4.} else {(ui.available_height() * 0.12).clamp(12., 60.)});
                        if !compact {
                        ui.add(
                            egui::Image::new(egui::include_image!(
                                "../../assets/app-icon/photosuite.png"
                            ))
                            .fit_to_exact_size(egui::vec2(64., 64.)),
                        );
                        ui.add_space(8.);
                        }
                        let unavailable=self.startup.as_ref().is_some_and(|s|s.state()==photosuite_application::preferences::StartupState::Unavailable);
                        ui.label(
                            egui::RichText::new(if unavailable{"Bibliothek aktuell nicht erreichbar."}else{"Platz für Erinnerungen."})
                                .size(if compact && unavailable {18.} else {22.})
                                .strong(),
                        );
                        ui.add_space(6.);
                        if !compact { ui.label(
                            egui::RichText::new("Deine Fotos. Deine Ordner. Deine Sammlung.")
                                .color(theme::MUTED),
                        ); }
                        ui.add_space(10.);
                        if unavailable {
                            if let Some(path)=self.startup.as_ref().and_then(|s|s.target()){ui.add(egui::Label::new(path.display().to_string()).truncate()).on_hover_text(path.display().to_string());}
                            if ui.button("Erneut versuchen").clicked(){self.retry_startup();}
                        }
                        if ui
                            .add_sized(
                                [ui.available_width().min(200.), 30.],
                                egui::Button::new(if unavailable{"Andere Bibliothek öffnen …"}else{"Fotosammlung öffnen …"})
                                    .fill(theme::SELECTED)
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                            )
                            .clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            self.load(path);
                        }
                        recent_selection = recent_libraries(ui, &self.preferences);
                        if !compact {
                        ui.add_space(6.);
                        ui.label(
                            egui::RichText::new("Lokal auf deinem Mac · Ohne Cloud oder Konto")
                                .small()
                                .color(theme::MUTED),
                        );
                        }
                    });
                    return;
                }
                let title = {
                    let group = self.group.as_deref().unwrap_or("Sammlung");
                    match group.split_once(std::path::MAIN_SEPARATOR) {
                        Some((year, "scans")) => format!("Scans {year}"),
                        Some((year, month)) if month.len() == 2 => {
                            format!("{} {year}", super::library::month_label(month))
                        }
                        _ => group.into(),
                    }
                };
                self.page_heading(ui, &title);
                self.prepare_gallery_selection();
                ui.small(format!("{} ausgewählt · Cmd-Klick: Mehrfachauswahl · Shift-Klick: Bereich · Doppelklick: Öffnen", self.gallery.selection.paths.len()));
                ui.add_space(4.);
                if visible.is_empty() && !self.collection.is_loading() {
                    ui.add_space(16.);
                    ui.label(
                        egui::RichText::new("Hier ist noch Platz für Erinnerungen.").size(17.),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Öffne einen anderen Ordner oder passe Suche und Filter an.",
                        )
                        .color(theme::MUTED),
                    );
                }
                ui.spacing_mut().item_spacing = egui::vec2(8., 6.);
                let target = [90., 130., 170., 240., 320.][self.size];
                let gap = ui.spacing().item_spacing.x;
                let width = (ui.available_width() - ui.spacing().scroll.allocated_width()).max(1.);
                let columns = ((width + gap) / (target + gap)).floor().max(1.) as usize;
                let tile = ((width - gap * (columns - 1) as f32) / columns as f32).max(1.);
                let row_height = tile + 36.;
                let mut scroll = egui::ScrollArea::vertical().id_salt((
                    "photo_wall",
                    &self.root,
                    &self.group,
                    &self.tag_view.filters,
                ));
                if let Some(path) = self.scroll_to_photo.take()
                    && let Some(position) = visible
                        .iter()
                        .position(|&index| self.items[index].path() == path)
                {
                    let row = position / columns;
                    let stride = row_height + ui.spacing().item_spacing.y;
                    let offset =
                        (row as f32 * stride - (ui.available_height() - stride) * 0.5).max(0.);
                    scroll = scroll.vertical_scroll_offset(offset);
                }
                scroll.show_rows(
                    ui,
                    row_height,
                    visible.len().div_ceil(columns),
                    |ui, rows| {
                        for row in rows {
                            ui.horizontal(|ui| {
                                ui.set_min_height(row_height);
                                for &index in visible.iter().skip(row * columns).take(columns) {
                                    let m = self.items[index].clone();
                                    let tags:Vec<String> = self.organization.tags_for(m.path()).into_iter().map(str::to_owned).collect();
                                    let mut tooltip = if tags.is_empty() {
                                        m.path().display().to_string()
                                    } else {
                                        format!("{}\nTags: {}", m.path().display(), tags.join(", "))
                                    };
                                    let mut has_geo=false;
                                    if m.is_photo() && let Some(root)=&self.root && let Ok(relative)=m.path().strip_prefix(root) && let Some(relative)=relative.to_str() && let Ok(file)=crate::domain::tags::LibraryFile::new(relative.into()) {
                                        match self.geo_markers.get(root,&file) {
                                            Some(Ok(snapshot))=>{has_geo=matches!(snapshot.position,photosuite_application::media_geo::Position::Present(_));},
                                            Some(Err(error))=>tooltip.push_str(&format!("\nGPS-Prüfung: {error}")),
                                            None=>{},
                                        }
                                    }
                                    if has_geo {tooltip.push_str("\nGPS-Koordinaten vorhanden · Rechtsklick → Ort anzeigen …");}
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 2.;
                                        ui.set_width(tile);
                                        let response = if m.is_audio() {
                                            super::diary::audio_preview(ui, tile)
                                        } else {
                                            let mut image = egui::Image::from_uri(preview_uri(
                                                m.path(),
                                                self.editor.rotations(),
                                                tile,
                                            ));
                                            let size = egui::vec2(tile, tile);
                                            if self.square_previews {
                                                if let Ok(egui::load::TexturePoll::Ready {
                                                    texture,
                                                }) = image.load_for_size(ui.ctx(), size)
                                                {
                                                    let dimensions = texture.size;
                                                    let mut uv = egui::Rect::from_min_max(
                                                        egui::pos2(0., 0.),
                                                        egui::pos2(1., 1.),
                                                    );
                                                    if dimensions.x > dimensions.y {
                                                        let inset =
                                                            (1. - dimensions.y / dimensions.x) / 2.;
                                                        uv.min.x = inset;
                                                        uv.max.x = 1. - inset;
                                                    } else if dimensions.y > dimensions.x {
                                                        let inset =
                                                            (1. - dimensions.x / dimensions.y) / 2.;
                                                        uv.min.y = inset;
                                                        uv.max.y = 1. - inset;
                                                    }
                                                    image = image.uv(uv);
                                                }
                                                image = image
                                                    .fit_to_exact_size(size)
                                                    .maintain_aspect_ratio(false);
                                            } else {
                                                image = image
                                                    .max_size(size)
                                                    .maintain_aspect_ratio(true);
                                            }
                                            let (rect, response) =
                                                ui.allocate_exact_size(size, egui::Sense::click());
                                            ui.painter().rect_filled(rect, 8., theme::SURFACE);
                                            let mut paint_rect = rect;
                                            if !self.square_previews
                                                && let Ok(egui::load::TexturePoll::Ready {
                                                    texture,
                                                }) = image.load_for_size(ui.ctx(), size)
                                            {
                                                let scale = (tile / texture.size.x)
                                                    .min(tile / texture.size.y);
                                                paint_rect = egui::Rect::from_center_size(
                                                    rect.center(),
                                                    texture.size * scale,
                                                );
                                            }
                                            theme::paint_image(
                                                ui,
                                                image.corner_radius(8),
                                                paint_rect,
                                            );
                                            let highlighted =
                                                self.highlighted_photo.as_deref() == Some(m.path());
                                            let selected = self.gallery.selection.paths.contains(m.path());
                                            if response.hovered() || highlighted || selected {
                                                ui.painter().rect_stroke(
                                                    rect.shrink(1.),
                                                    8.,
                                                    egui::Stroke::new(
                                                        if highlighted || selected { 3_f32 } else { 2_f32 },
                                                        theme::ACCENT,
                                                    ),
                                                    egui::StrokeKind::Inside,
                                                );
                                            }
                                            if selected {
                                                let center = rect.right_top() + egui::vec2(-12.,12.);
                                                ui.painter().circle_filled(center,8.,theme::ACCENT);
                                                ui.painter().line_segment([center+egui::vec2(-4.,0.),center+egui::vec2(-1.,3.)],egui::Stroke::new(2_f32,theme::CANVAS));
                                                ui.painter().line_segment([center+egui::vec2(-1.,3.),center+egui::vec2(4.,-3.)],egui::Stroke::new(2_f32,theme::CANVAS));
                                            }
                                            let mut badges = Vec::with_capacity(tags.len() + 1);
                                            if m.is_video() {
                                                badges.push("Video");
                                            }
                                            badges.extend(tags.iter().map(String::as_str));
                                            if let Some(geo_rect)=super::badges::paint_cloud(ui, paint_rect, &badges,has_geo)
                                                && ui.interact(geo_rect,response.id.with("geo_location"),egui::Sense::click()).on_hover_text("GPS-Koordinaten vorhanden · Ort anzeigen").clicked(){self.open_photo_location(&m);}

                                            response.on_hover_cursor(egui::CursorIcon::PointingHand)
                                        };
                                        if response.secondary_clicked() {self.gallery.selection.context(m.path());}
                                        response.context_menu(|ui| {
                                            self.gallery.selection.context(m.path());
                                            let count=self.gallery.selection.paths.len();
                                            if ui.button("Im Finder zeigen …").clicked() {
                                                if let Err(error)=self.services.opener.reveal(m.path()) {self.status=error;}
                                                ui.close();
                                            }
                                            if count==1 && !m.is_audio() && ui.add_enabled(!self.date_corrections.busy() && !self.comments.is_open(),egui::Button::new("Datum korrigieren …")).clicked() {
                                                self.execute_for(crate::commands::Command::CorrectDate,index,ctx);
                                                ui.close();
                                            }
                                            if count==1 && m.is_photo()
                                                && ui.button("Audiokommentar …").clicked()
                                            {
                                                self.record_photo = Some(index);
                                                ui.close();
                                            }
                                            if count==1 && m.is_photo() && ui.button("Ort anzeigen …").clicked(){self.open_photo_location(&m);ui.close();}
                                            if ui.button(format!("Tags bearbeiten … ({count})")).clicked() {
                                                let files:Result<Vec<_>,_>=self.gallery.selection.paths.iter().map(|path|self.organization.file(path)).collect();
                                                match files {Ok(files)=>self.tag_view.edit_many(files),Err(error)=>self.status=error}
                                                ui.close();
                                            }
                                            ui.separator();
                                            let label=if count>1 {format!("Auswahl löschen … ({count})")}else if m.is_video(){"Video löschen …".into()}else{"Bild löschen …".into()};
                                            if ui.add_enabled(!self.gallery.job.running() && !self.comments.is_open() && !self.editor.is_saving(),egui::Button::new(egui::RichText::new(label).color(theme::DANGER))).clicked(){
                                                if let Some(root)=&self.root {
                                                    let files:Result<Vec<_>,_>=self.gallery.selection.paths.iter().map(|path|self.organization.file(path)).collect();
                                                    match files {Ok(files)=>self.gallery.pending=Some((root.clone(),files)),Err(error)=>self.status=error}
                                                }
                                                ui.close();
                                            }
                                        });
                                        let clicked=response.clicked();
                                        let double_clicked=response.double_clicked();
                                        response.on_hover_text(&tooltip);
                                        if clicked {
                                            let modifiers=ui.input(|i|i.modifiers);
                                            let ordered:Vec<_>=visible.iter().map(|&index|self.items[index].path().to_path_buf()).collect();
                                            self.gallery.selection.click(m.path(),&ordered,modifiers.command || modifiers.ctrl,modifiers.shift);
                                        }
                                        if double_clicked && !ui.input(|i|i.modifiers.command || i.modifiers.ctrl || i.modifiers.shift) {
                                            if m.is_audio() {
                                                if let Err(error) = self.diary.start(
                                                    &self.items,
                                                    &m,
                                                    self.services.audio_source.clone(),
                                                ) {
                                                    self.status = error;
                                                }
                                            } else {
                                                open_media = Some(index);
                                            }
                                        }
                                        super::captions::caption(
                                            ui,
                                            &mut self.media_dates,
                                            &m,
                                            tile,
                                        );
                                    });
                                }
                            });
                        }
                    },
                );
            });
        if let Some(path) = recent_selection {
            self.load(path);
        }
        if let Some(index) = open_media {
            self.navigate_viewer(index);
        }
    }
}

#[cfg(test)]
mod recent_tests {
    use super::*;
    use photosuite_application::preferences::Preferences;
    fn preferences() -> Preferences {
        let mut p = Preferences::default();
        p.remember_library("/Offline/current".into());
        p.set_recent_libraries((0..5).map(|i| format!("/Offline/Library {i}").into()));
        p
    }
    #[test]
    fn all_five_shortcuts_fit_the_small_start_page_without_a_scroll_area() {
        for (width, height) in [(360., 400.), (600., 500.), (1280., 850.)] {
            let ctx = egui::Context::default();
            crate::theme::install(&ctx);
            let mut suite = super::super::layout_tests::test_suite();
            suite.root = None;
            suite.preferences = preferences();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, height),
                    )),
                    ..Default::default()
                },
                |ctx| suite.photo_wall(ctx, &[]),
            );
            let labels = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text().starts_with("Library ") => {
                        Some(text)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(labels.len(), 5);
            for label in labels {
                let rect = label.visual_bounding_rect();
                assert!(rect.bottom() < height, "shortcut clipped: {rect:?}");
                assert!(rect.right() <= width, "shortcut too wide: {rect:?}");
            }
        }
    }
    #[test]
    fn clicking_a_recent_library_returns_its_complete_offline_path() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        ctx.style_mut(|s| s.animation_time = 0.);
        let preferences = preferences();
        let mut selected = None;
        let mut frame = |events| {
            ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            if let Some(path) = recent_libraries(ui, &preferences) {
                                selected = Some(path);
                            }
                        });
                    });
                },
            )
        };
        let output = frame(vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text().starts_with("Library 3") => {
                    Some(text.visual_bounding_rect().center())
                }
                _ => None,
            })
            .unwrap();
        for pressed in [true, false] {
            let _ = frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
        assert_eq!(selected, Some("/Offline/Library 3".into()));
    }
}
