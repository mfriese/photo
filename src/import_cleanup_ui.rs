//! Confirmation UI only; verification and execution live in the application session.
use eframe::egui;
use photosuite_application::import_cleanup::{CleanupPort, CleanupRequest, CleanupSession, Status};
use std::sync::Arc;
#[derive(Default)]
pub struct SourceCleanupDialog {
    visible: bool,
    session: CleanupSession,
    error: Option<String>,
}
impl SourceCleanupDialog {
    pub fn busy(&self) -> bool {
        self.visible || self.session.running()
    }
    pub fn open(&mut self, request: CleanupRequest, port: Arc<dyn CleanupPort>) {
        self.error = self.session.start(request, port).err();
        self.visible = true;
    }
    pub fn show(&mut self, ctx: &egui::Context, port: Arc<dyn CleanupPort>) {
        self.session.poll();
        if self.session.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        let was_visible = self.visible;
        let mut close = false;
        egui::Window::new("Importquellen in den Papierkorb")
            .open(&mut self.visible).default_size([820.,560.]).vscroll(true)
            .max_width((ctx.content_rect().width()-32.).max(280.))
            .max_height((ctx.content_rect().height()-32.).max(280.))
            .show(ctx,|ui|{
                crate::options_ui::spacing(ui);
                ui.label("Separater Schritt nach dem Import. Es werden ausschließlich Quelldateien verschoben – niemals Zieldateien oder ganze Verzeichnisse.");
                ui.small("Freigabe nur nach vollständigem Bytevergleich. Vor jeder Papierkorbaktion werden beide Dateien und ihre geprüften Dateistände erneut kontrolliert.");
                ui.label(self.session.status());
                if let Some(error)=&self.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
                let checked=self.session.rows().iter().filter(|r|r.status!=Status::Pending).count();
                if self.session.running() && !self.session.rows().is_empty(){ui.add(egui::ProgressBar::new(checked as f32/self.session.rows().len() as f32).text(format!("{checked} / {} geprüft",self.session.rows().len())));}
                egui::ScrollArea::vertical().max_height((ui.available_height()-100.).max(80.)).show(ui,|ui|{
                    for row in self.session.rows(){
                        let (label,color)=match &row.status{
                            Status::Pending=>("Wird geprüft".into(),crate::theme::MUTED),
                            Status::Ready=>("Freigegeben · Ziel vorhanden und byteidentisch".into(),crate::theme::TEXT),
                            Status::Blocked(reason)=>(format!("Bleibt erhalten · {reason}"),egui::Color32::LIGHT_RED),
                            Status::Moving=>("Wird erneut geprüft / verschoben …".into(),crate::theme::TEXT),
                            Status::Unknown(reason)=>(format!("Status unklar · {reason}"),egui::Color32::LIGHT_RED),
                            Status::Trashed=>("In den Papierkorb verschoben".into(),crate::theme::TEXT),
                            Status::Kept(reason)=>(format!("Nicht verschoben · {reason}"),egui::Color32::LIGHT_RED),
                        };
                        egui::Frame::group(ui.style()).show(ui,|ui|{
                            ui.set_width(ui.available_width());
                            ui.label(egui::RichText::new(row.source.file_name().unwrap_or_default().to_string_lossy()).strong());
                            ui.add(egui::Label::new(format!("Quelle: {}",row.source.display())).truncate()).on_hover_text(row.source.display().to_string());
                            let target=row.target.as_ref().map(|p|p.display().to_string()).unwrap_or_else(||"Kein Ziel".into());
                            ui.add(egui::Label::new(format!("Ziel: {target}")).truncate()).on_hover_text(target);
                            let imported=match row.decision{photosuite_application::import::FileDecision::Copied=>"Import: kopiert",photosuite_application::import::FileDecision::SkippedIdentical=>"Import: bereits identisch vorhanden",photosuite_application::import::FileDecision::SkippedVideoName=>"Import: nur Videoname vorhanden; Inhalt wird hier erneut geprüft",_=>"Import: nicht erfolgreich kopiert"};
                            ui.small(imported);ui.colored_label(color,label);
                        });
                    }
                });
                ui.separator();
                ui.horizontal_wrapped(|ui|{
                    if ui.button(if self.session.running(){"Abbrechen"}else{"Schließen / behalten"}).clicked(){if !self.session.running(){close=true;}self.session.cancel();}
                    let count=self.session.ready_count();
                    if ui.add_enabled(self.session.can_confirm(),egui::Button::new(format!("{count} freigegebene Quellen in den Papierkorb"))).clicked(){self.error=self.session.confirm(port.clone()).err();}
                });
                ui.small("Nicht geprüfte, abweichende und fehlgeschlagene Dateien bleiben erhalten. Bereits verschobene Dateien bleiben bei Abbruch im Papierkorb. Nicht bearbeitete Quellen sind nicht Teil dieser Liste und werden niemals entfernt.");
            });
        if close {
            self.visible = false;
        }
        if was_visible && !self.visible {
            self.session.cancel();
        }
    }
}
