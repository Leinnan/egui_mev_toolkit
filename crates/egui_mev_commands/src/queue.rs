//! Commands waiting for the app to run them, kept per [`Context`].

use egui::{Context, Id};

/// A per-context queue of commands: menus, palettes and event handlers push, and
/// the app drains once a frame. It lives in the context's data, so each window
/// harness in a test has its own, and each command type has a separate queue.
#[derive(Clone, Debug)]
pub struct CommandQueue<C>(Vec<C>);

impl<C> Default for CommandQueue<C> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<C: Clone + Send + Sync + 'static> CommandQueue<C> {
    fn id() -> Id {
        Id::unique("egui_mev_commands::CommandQueue")
    }

    /// Queue `command` and ask for a frame to run it in, so a push from outside a
    /// frame or late in one doesn't wait for the next input.
    pub fn push(ctx: &Context, command: C) {
        ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<Self>(Self::id())
                .0
                .push(command);
        });
        ctx.request_repaint();
    }

    /// The queued commands, oldest first, leaving the queue empty.
    #[must_use]
    pub fn drain(ctx: &Context) -> Vec<C> {
        ctx.data_mut(|data| std::mem::take(&mut data.get_temp_mut_or_default::<Self>(Self::id()).0))
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    #[test]
    fn drains_in_order_and_leaves_it_empty() {
        let ctx = Context::default();
        CommandQueue::push(&ctx, 1);
        CommandQueue::push(&ctx, 2);
        assert_eq!(CommandQueue::<i32>::drain(&ctx), vec![1, 2]);
        assert_eq!(CommandQueue::<i32>::drain(&ctx), Vec::<i32>::new());
    }

    #[test]
    fn contexts_do_not_share_a_queue() {
        let (a, b) = (Context::default(), Context::default());
        CommandQueue::push(&a, "a");
        CommandQueue::push(&b, "b");
        assert_eq!(CommandQueue::<&str>::drain(&a), vec!["a"]);
        assert_eq!(CommandQueue::<&str>::drain(&b), vec!["b"]);
    }

    #[test]
    fn command_types_do_not_share_a_queue() {
        let ctx = Context::default();
        CommandQueue::push(&ctx, 1_u8);
        CommandQueue::push(&ctx, "text");
        assert_eq!(CommandQueue::<u8>::drain(&ctx), vec![1]);
        assert_eq!(CommandQueue::<&str>::drain(&ctx), vec!["text"]);
    }

    /// What used to need a thread-local: harnesses running in parallel only ever
    /// see their own clicks.
    #[test]
    fn parallel_harnesses_stay_isolated() {
        let threads: Vec<_> = (0..4_u32)
            .map(|index| {
                std::thread::spawn(move || {
                    let mut harness = Harness::new_ui(move |ui| {
                        if ui.button("Run").clicked() {
                            CommandQueue::push(ui.ctx(), index);
                        }
                    });
                    let mut seen = Vec::new();
                    for _ in 0..5 {
                        harness.run();
                        harness.get_by_label("Run").click();
                        harness.run();
                        seen.extend(CommandQueue::<u32>::drain(&harness.ctx));
                    }
                    (index, seen)
                })
            })
            .collect();
        for thread in threads {
            let (index, seen) = thread.join().expect("the harness thread finishes");
            assert_eq!(seen, vec![index; 5]);
        }
    }

    #[test]
    fn a_push_asks_for_a_frame() {
        let ctx = Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        CommandQueue::push(&ctx, 1);
        assert!(ctx.has_requested_repaint());
    }
}
