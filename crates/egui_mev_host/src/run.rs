//! The simple case: one window, for as long as it stays open.

use std::sync::Mutex;
use std::time::Duration;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::WindowId;

use crate::{HostApp, HostError, HostOptions, HostedWindow};

/// Opens a window for `app` and runs until it closes, by the OS asking or by the
/// app sending [`egui::ViewportCommand::Close`]. Call it from the main thread.
///
/// # Errors
///
/// Returns an error if the event loop, window or renderer can't be created.
pub fn run<A: HostApp>(app: A, options: HostOptions) -> Result<(), HostError> {
    let event_loop = EventLoop::<RepaintAfter>::with_user_event().build()?;
    let mut host = Host {
        starting: Some((app, options)),
        window: None,
        proxy: Some(event_loop.create_proxy()),
        error: None,
    };
    event_loop.run_app(&mut host)?;
    host.error.map_or(Ok(()), Err)
}

/// Sent to the loop when anything asks egui for a repaint, from any thread.
struct RepaintAfter(Duration);

struct Host<A> {
    starting: Option<(A, HostOptions)>,
    window: Option<HostedWindow<A>>,
    proxy: Option<EventLoopProxy<RepaintAfter>>,
    error: Option<HostError>,
}

impl<A: HostApp> ApplicationHandler<RepaintAfter> for Host<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some((app, options)) = self.starting.take() else {
            return;
        };
        match HostedWindow::new(event_loop, app, options) {
            Ok(window) => {
                if let Some(proxy) = self.proxy.take() {
                    // The proxy isn't `Sync` on every system.
                    let proxy = Mutex::new(proxy);
                    window.context().set_request_repaint_callback(move |info| {
                        if let Ok(proxy) = proxy.lock() {
                            // Only fails once the loop is gone.
                            let _ = proxy.send_event(RepaintAfter(info.delay));
                        }
                    });
                }
                self.window = Some(window);
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: RepaintAfter) {
        if let Some(window) = &mut self.window {
            window.request_repaint_after(event.0);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_mut().filter(|w| w.window_id() == window_id) else {
            return;
        };
        let close = if matches!(event, WindowEvent::RedrawRequested) {
            window.paint();
            window.close_requested()
        } else {
            window.handle_event(&event)
        };
        if close {
            self.window = None;
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = &mut self.window {
            window.schedule_repaint(event_loop);
        }
    }
}
