//! Parented native sheets are created and polled on the app's main thread.
//! No modal wait or file I/O runs inside the controls/render callback.

use crate::presets::{Profile, ProfileStore};
use eframe::egui;
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, mpsc},
    task::{Context, Poll, Wake, Waker},
};

enum Request {
    Import,
    Export(Box<Profile>),
}

pub(super) enum Outcome {
    Imported(Box<Profile>),
    Exported,
    Cancelled,
    Failed(String),
}

type Picker = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

enum Active {
    Choosing { request: Request, picker: Picker },
    Working(mpsc::Receiver<Outcome>),
}

#[derive(Default)]
pub(super) struct ProfileDialog {
    queued: Option<Request>,
    active: Option<Active>,
}

struct Repaint(egui::Context);

impl Wake for Repaint {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint();
    }
}

impl ProfileDialog {
    pub(super) fn is_busy(&self) -> bool {
        self.queued.is_some() || self.active.is_some()
    }

    pub(super) fn import(&mut self) -> bool {
        self.queue(Request::Import)
    }

    pub(super) fn export(&mut self, snapshot: Profile) -> bool {
        self.queue(Request::Export(Box::new(snapshot)))
    }

    fn queue(&mut self, request: Request) -> bool {
        if self.is_busy() {
            return false;
        }
        self.queued = Some(request);
        true
    }

    /// Called before drawing controls, on a later main-thread UI pass than
    /// the button click. rfd uses AppKit's asynchronous parented-sheet API
    /// here; a worker must not construct a macOS native panel.
    pub(super) fn advance(
        &mut self,
        frame: &eframe::Frame,
        ctx: &egui::Context,
    ) -> Option<Outcome> {
        if let Some(request) = self.queued.take() {
            let dialog = rfd::AsyncFileDialog::new()
                .set_parent(frame)
                .add_filter("Visualiser profile", &["json"]);
            let picker: Picker = match &request {
                Request::Import => {
                    let future = dialog.set_title("Import visualiser profile").pick_file();
                    Box::pin(async move { future.await.map(|file| file.path().to_owned()) })
                }
                Request::Export(_) => {
                    let future = dialog
                        .set_title("Export visualiser profile")
                        .set_file_name("amara-profile.json")
                        .save_file();
                    Box::pin(async move { future.await.map(|file| file.path().to_owned()) })
                }
            };
            self.active = Some(Active::Choosing { request, picker });
        }
        self.poll_active(ctx)
    }

    fn poll_active(&mut self, ctx: &egui::Context) -> Option<Outcome> {
        match self.active.take()? {
            Active::Choosing {
                request,
                mut picker,
            } => {
                let waker = Waker::from(Arc::new(Repaint(ctx.clone())));
                match picker.as_mut().poll(&mut Context::from_waker(&waker)) {
                    Poll::Pending => {
                        self.active = Some(Active::Choosing { request, picker });
                        None
                    }
                    Poll::Ready(None) => Some(Outcome::Cancelled),
                    Poll::Ready(Some(path)) => self.begin_file_work(request, path, ctx),
                }
            }
            Active::Working(receiver) => match receiver.try_recv() {
                Ok(outcome) => Some(outcome),
                Err(mpsc::TryRecvError::Empty) => {
                    self.active = Some(Active::Working(receiver));
                    None
                }
                Err(mpsc::TryRecvError::Disconnected) => Some(Outcome::Failed(
                    "The profile file operation ended unexpectedly. Please try again.".into(),
                )),
            },
        }
    }

    fn begin_file_work(
        &mut self,
        request: Request,
        path: PathBuf,
        ctx: &egui::Context,
    ) -> Option<Outcome> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("amara-profile-file".into())
            .spawn(move || {
                let outcome = match request {
                    Request::Import => match ProfileStore::import(&path) {
                        Ok(profile) => Outcome::Imported(Box::new(profile)),
                        Err(error) => Outcome::Failed(format!("Could not import: {error}")),
                    },
                    Request::Export(profile) => match ProfileStore::export(&profile, &path) {
                        Ok(()) => Outcome::Exported,
                        Err(error) => Outcome::Failed(format!("Could not export: {error}")),
                    },
                };
                let _ = sender.send(outcome);
                ctx.request_repaint();
            }) {
            Ok(_) => {
                self.active = Some(Active::Working(receiver));
                None
            }
            Err(error) => Some(Outcome::Failed(format!(
                "Could not start the profile file operation: {error}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn one_request_at_a_time_retains_the_clicked_export_snapshot() {
        let mut dialog = ProfileDialog::default();
        let mut profile = Profile::astral_bloom();
        profile.name = "At the click".into();
        assert!(dialog.export(profile.clone()));
        profile.name = "Changed while choosing a file".into();
        assert!(!dialog.import());
        assert!(!dialog.export(profile));
        let Request::Export(snapshot) = dialog.queued.take().unwrap() else {
            panic!("queued export must retain its snapshot");
        };
        assert_eq!(snapshot.name, "At the click");
    }

    #[test]
    fn pending_picker_never_waits_and_cancellation_releases_the_slot() {
        let ctx = egui::Context::default();
        let mut dialog = ProfileDialog {
            queued: None,
            active: Some(Active::Choosing {
                request: Request::Import,
                picker: Box::pin(std::future::pending()),
            }),
        };
        for _ in 0..100 {
            assert!(dialog.poll_active(&ctx).is_none());
            assert!(dialog.is_busy());
        }
        assert!(!dialog.import());
        dialog.active = Some(Active::Choosing {
            request: Request::Import,
            picker: Box::pin(std::future::ready(None)),
        });
        assert!(matches!(dialog.poll_active(&ctx), Some(Outcome::Cancelled)));
        assert!(!dialog.is_busy());
        assert!(dialog.import());
    }

    fn await_work(dialog: &mut ProfileDialog, ctx: &egui::Context) -> Outcome {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(outcome) = dialog.poll_active(ctx) {
                return outcome;
            }
            assert!(Instant::now() < deadline, "profile worker did not finish");
            std::thread::yield_now();
        }
    }

    #[test]
    fn background_file_work_round_trips_the_selected_snapshot() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "amara-profile-dialog-test-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("profile.json");
        let ctx = egui::Context::default();
        let mut dialog = ProfileDialog::default();
        let mut profile = Profile::astral_bloom();
        profile.name = "Async snapshot".into();
        assert!(
            dialog
                .begin_file_work(
                    Request::Export(Box::new(profile.clone())),
                    path.clone(),
                    &ctx
                )
                .is_none()
        );
        assert!(dialog.is_busy());
        assert!(!dialog.import());
        assert!(matches!(await_work(&mut dialog, &ctx), Outcome::Exported));
        assert!(!dialog.is_busy());
        assert!(
            dialog
                .begin_file_work(Request::Import, path.clone(), &ctx)
                .is_none()
        );
        let Outcome::Imported(imported) = await_work(&mut dialog, &ctx) else {
            panic!("valid exported profile must import");
        };
        assert_eq!(*imported, profile);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn file_failure_or_lost_worker_does_not_leave_dialog_busy() {
        let ctx = egui::Context::default();
        let mut dialog = ProfileDialog::default();
        assert!(
            dialog
                .begin_file_work(Request::Import, std::env::temp_dir(), &ctx)
                .is_none()
        );
        assert!(matches!(await_work(&mut dialog, &ctx), Outcome::Failed(_)));
        assert!(!dialog.is_busy());
        let (sender, receiver) = mpsc::channel();
        dialog.active = Some(Active::Working(receiver));
        drop(sender);
        assert!(matches!(dialog.poll_active(&ctx), Some(Outcome::Failed(_))));
        assert!(!dialog.is_busy());
        assert!(dialog.import());
    }
}
