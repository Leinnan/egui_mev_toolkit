//! One egui window on `egui-winit` and `egui-wgpu`.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{ViewportCommand, ViewportId, ViewportInfo};
use egui_mev_app::{NativeChrome, wants_mica};
use egui_mev_theme::{AccentShades, Palette};
use egui_wgpu::winit::Painter;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

use crate::{HostApp, HostError, HostOptions};

/// An egui window for an app with its own event loop.
///
/// Create it from a running loop, then forward that window's events: call
/// [`Self::paint`] for `RedrawRequested`, [`Self::handle_event`] for the rest, and
/// [`Self::schedule_repaint`] from `about_to_wait`. To have other threads'
/// [`egui::Context::request_repaint`] wake the loop, set a callback on
/// [`Self::context`] that reaches your loop and calls [`Self::request_repaint_after`].
pub struct HostedWindow<A> {
    window: Arc<Window>,
    egui_state: egui_winit::State,
    egui_ctx: egui::Context,
    painter: Painter,
    app: A,
    viewport_id: ViewportId,
    viewport_info: ViewportInfo,
    chrome: NativeChrome,
    repaint_at: Option<Instant>,
    close_requested: bool,
}

impl<A: HostApp> HostedWindow<A> {
    /// Creates the window on `event_loop` and installs the options' theme and fonts.
    ///
    /// # Errors
    ///
    /// Returns an error if the window or the renderer can't be created.
    pub fn new(
        event_loop: &ActiveEventLoop,
        app: A,
        options: HostOptions,
    ) -> Result<Self, HostError> {
        let mica = wants_mica(options.theme.family);
        let attributes = window_attributes(&options, mica);
        let window = Arc::new(event_loop.create_window(attributes)?);

        let egui_ctx = egui::Context::default();
        options.theme.install_with_fonts(&egui_ctx, &options.fonts);

        let viewport_id = ViewportId::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            viewport_id,
            event_loop,
            None,
            event_loop.system_theme(),
            None,
        );

        let mut viewport_info = ViewportInfo::default();
        egui_winit::update_viewport_info(&mut viewport_info, &egui_ctx, &window, true);

        let painter = create_painter(&egui_ctx, viewport_id, &window, mica)?;

        window.focus_window();

        let accent = AccentShades::system();
        Ok(Self {
            window,
            egui_state,
            egui_ctx,
            painter,
            app,
            viewport_id,
            viewport_info,
            chrome: if mica {
                NativeChrome::transparent(accent)
            } else {
                NativeChrome::new(accent)
            },
            repaint_at: Some(Instant::now()),
            close_requested: false,
        })
    }

    /// The id to match against the loop's window events.
    #[must_use]
    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    /// The native window, for changing its title or size later.
    #[must_use]
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// The window's egui context.
    #[must_use]
    pub const fn context(&self) -> &egui::Context {
        &self.egui_ctx
    }

    /// Handles a window event other than `RedrawRequested`, which goes to
    /// [`Self::paint`].
    ///
    /// Returns `true` if the window should close. The window itself stays open
    /// until the caller drops it.
    pub fn handle_event(&mut self, event: &WindowEvent) -> bool {
        if let WindowEvent::Resized(size) = event
            && let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        {
            self.painter
                .on_window_resized(self.viewport_id, width, height);
        }

        let response = self.egui_state.on_window_event(&self.window, event);
        if response.repaint {
            self.window.request_redraw();
        }
        matches!(event, WindowEvent::CloseRequested)
    }

    /// Runs one egui frame and draws it. Check [`Self::close_requested`] after.
    pub fn paint(&mut self) {
        egui_winit::update_viewport_info(
            &mut self.viewport_info,
            &self.egui_ctx,
            &self.window,
            false,
        );
        let mut raw_input = self.egui_state.take_egui_input(&self.window);
        raw_input
            .viewports
            .insert(self.viewport_id, self.viewport_info.clone());

        // Before the frame, so the first one already draws with the theme the
        // backdrop calls for, and no opaque frame flashes before Mica.
        self.chrome.sync(&self.egui_ctx, &*self.window);

        let app = &mut self.app;
        let full_output = self.egui_ctx.run_ui(raw_input, |ui| app.ui(ui));
        self.paint_full_output(full_output);
    }

    /// Raises the window and draws it.
    pub fn focus(&self) {
        self.window.focus_window();
        self.window.request_redraw();
    }

    /// Whether the app asked to close with [`ViewportCommand::Close`], as
    /// [`Self::handle_event`] reports for the OS asking.
    #[must_use]
    pub const fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// Asks for a frame after `delay` at the latest, on top of what egui asked
    /// for. The event loop picks it up at the next [`Self::schedule_repaint`].
    pub fn request_repaint_after(&mut self, delay: Duration) {
        if let Some(deadline) = Instant::now().checked_add(delay) {
            self.repaint_at = Some(self.repaint_at.map_or(deadline, |at| at.min(deadline)));
        }
    }

    /// Schedules the next egui frame, which may need no native input to happen.
    pub fn schedule_repaint(&mut self, event_loop: &ActiveEventLoop) {
        let (redraw, flow) = repaint_control_flow(self.repaint_at, Instant::now());
        event_loop.set_control_flow(flow);
        if redraw {
            self.window.request_redraw();
            self.repaint_at = None;
        }
    }

    fn paint_full_output(&mut self, mut full_output: egui::FullOutput) {
        // Again after the frame, which may have picked up a new OS theme.
        self.chrome.sync(&self.egui_ctx, &*self.window);
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        // Window commands, such as title bar dragging and zooming.
        if let Some(viewport_output) = full_output.viewport_output.remove(&self.viewport_id) {
            self.repaint_at = Instant::now().checked_add(viewport_output.repaint_delay);
            self.close_requested |= viewport_output.commands.contains(&ViewportCommand::Close);
            let mut actions_requested = Vec::new();
            egui_winit::process_viewport_commands(
                &self.egui_ctx,
                &mut self.viewport_info,
                viewport_output.commands,
                &self.window,
                &mut actions_requested,
            );
            // Commands queue these for the next frame's input, and nothing here
            // reads them there: this window handles closing above.
            self.viewport_info.events.clear();
        }

        let clipped_primitives = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let clear_color = self.chrome.clear_color(&self.egui_ctx).unwrap_or_else(|| {
            Palette::of_ctx(&self.egui_ctx)
                .panel_bg
                .to_normalized_gamma_f32()
        });

        self.painter.paint_and_update_textures(
            self.viewport_id,
            full_output.pixels_per_point,
            clear_color,
            &clipped_primitives,
            &mut full_output.textures_delta,
            vec![],
            &self.window,
        );
    }
}

impl<A> Drop for HostedWindow<A> {
    fn drop(&mut self) {
        self.painter.destroy();
    }
}

fn window_attributes(options: &HostOptions, mica: bool) -> WindowAttributes {
    let [width, height] = options.inner_size;
    let [min_width, min_height] = options.min_inner_size;
    let attributes = Window::default_attributes()
        .with_title(&options.title)
        .with_window_icon(options.icon.as_ref().and_then(window_icon))
        .with_inner_size(LogicalSize::new(width, height))
        .with_min_inner_size(LogicalSize::new(min_width, min_height));
    #[cfg(target_os = "macos")]
    let attributes = {
        use winit::platform::macos::WindowAttributesExtMacOS as _;
        attributes
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true)
    };
    #[cfg(windows)]
    let attributes = {
        use winit::platform::windows::WindowAttributesExtWindows as _;
        attributes
            .with_transparent(mica)
            .with_no_redirection_bitmap(mica)
    };
    #[cfg(not(windows))]
    let _ = mica;
    attributes
}

fn window_icon(icon: &egui::IconData) -> Option<Icon> {
    Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height)
        .inspect_err(|error| log::warn!("Ignoring the window icon: {error}"))
        .ok()
}

/// Creates the renderer. On Windows it retries with DX12 WARP or OpenGL, the only
/// option on Windows 7 and 8, which have no DirectX 12.
///
/// With `transparent` (Mica) the swapchain supports per-pixel alpha through
/// `DirectComposition`; see [`egui_mev_app::mica_wgpu_setup`].
fn create_painter(
    egui_ctx: &egui::Context,
    viewport_id: ViewportId,
    window: &Arc<Window>,
    transparent: bool,
) -> Result<Painter, HostError> {
    #[cfg(windows)]
    let configuration = if transparent {
        egui_wgpu::WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(egui_mev_app::mica_wgpu_setup()),
            ..egui_wgpu::WgpuConfiguration::default()
        }
    } else {
        egui_wgpu::WgpuConfiguration::default()
    };
    #[cfg(not(windows))]
    let configuration = egui_wgpu::WgpuConfiguration::default();
    let mut painter = pollster::block_on(Painter::new(
        egui_ctx.clone(),
        configuration,
        transparent,
        egui_wgpu::RendererOptions::default(),
    ));

    if let Err(hardware_error) =
        pollster::block_on(painter.set_window(viewport_id, Some(window.clone())))
    {
        #[cfg(not(windows))]
        return Err(hardware_error.into());
        #[cfg(windows)]
        {
            log::warn!("Retrying with a software graphics adapter: {hardware_error}");
            drop(painter);
            let mut setup = if transparent {
                egui_mev_app::mica_wgpu_setup()
            } else {
                egui_wgpu::WgpuSetupCreateNew::without_display_handle()
            };
            // DX12 WARP needs Windows 10. OpenGL covers older systems and
            // machines without a DX12-capable driver.
            setup.instance_descriptor.backends = if transparent {
                wgpu::Backends::DX12
            } else {
                wgpu::Backends::DX12 | wgpu::Backends::GL
            };
            setup.native_adapter_selector = Some(Arc::new(|adapters, surface| {
                adapters
                    .iter()
                    .filter(|adapter| {
                        surface.is_none_or(|surface| adapter.is_surface_supported(surface))
                    })
                    .min_by_key(|adapter| {
                        let info = adapter.get_info();
                        // Prefer DX12 WARP, then any OpenGL adapter.
                        (
                            info.device_type != wgpu::DeviceType::Cpu,
                            info.backend != wgpu::Backend::Dx12,
                        )
                    })
                    .cloned()
                    .ok_or_else(|| {
                        "No compatible fallback graphics adapter is available".to_owned()
                    })
            }));
            painter = pollster::block_on(Painter::new(
                egui_ctx.clone(),
                egui_wgpu::WgpuConfiguration {
                    wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(setup),
                    ..egui_wgpu::WgpuConfiguration::default()
                },
                transparent,
                egui_wgpu::RendererOptions::default(),
            ));
            pollster::block_on(painter.set_window(viewport_id, Some(window.clone()))).map_err(
                |software_error| {
                    HostError::WgpuInit(format!(
                        "graphics initialization failed: {hardware_error}. \
                         Software fallback also failed: {software_error}"
                    ))
                },
            )?;
        }
    }

    Ok(painter)
}

fn repaint_control_flow(deadline: Option<Instant>, now: Instant) -> (bool, ControlFlow) {
    match deadline {
        Some(deadline) if deadline > now => (false, ControlFlow::WaitUntil(deadline)),
        Some(_) => (true, ControlFlow::Wait),
        None => (false, ControlFlow::Wait),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use egui::IconData;
    use winit::event_loop::ControlFlow;

    use super::{repaint_control_flow, window_icon};

    #[test]
    fn repaint_deadlines_wake_idle_event_loop() {
        let now = Instant::now();
        let deadline = now + Duration::from_millis(50);
        assert_eq!(
            repaint_control_flow(Some(deadline), now),
            (false, ControlFlow::WaitUntil(deadline))
        );
        assert_eq!(
            repaint_control_flow(Some(deadline), deadline),
            (true, ControlFlow::Wait)
        );
        assert_eq!(
            repaint_control_flow(Some(deadline), deadline + Duration::from_millis(1)),
            (true, ControlFlow::Wait)
        );
        assert_eq!(repaint_control_flow(None, now), (false, ControlFlow::Wait));
    }

    #[test]
    fn a_valid_icon_converts_and_a_bad_one_is_dropped() {
        let icon = |rgba: Vec<u8>| IconData {
            rgba,
            width: 1,
            height: 1,
        };
        assert!(window_icon(&icon(vec![0, 0, 0, 255])).is_some());
        assert!(window_icon(&icon(vec![0, 0, 0])).is_none());
    }
}
