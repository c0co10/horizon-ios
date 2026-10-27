//! Splitscreen on an AirPlay display (iPhone and iPad; Horizon's own): the game on the TV is played with a
//! controller, and a second player plays on the phone with the touch controls. The second player is a second
//! client `Game` joined to the window's integrated server through memory, as a LAN guest joins it, with its
//! own renderer on the phone's window, no sound (the first player's plays) and no controller. The pause menu
//! starts and ends it (game/screens.rs); the second player leaving the world, the first leaving theirs, or
//! the AirPlay display going away ends it too.
use super::{Game, UiResources};
use horizon_client::{Result, audio::Audio, hud::HudAtlas};
use std::sync::Arc;
use std::time::Instant;
use winit::event::WindowEvent;

#[derive(Default)]
pub(super) struct Splitscreen {
    /// This game is the second player.
    pub guest: bool,
    /// The second player plays alongside this one.
    pub active: bool,
    /// The pause menu asked to start (true) or end (false) it.
    pub request: Option<bool>,
}

/// The second player's name: the first player's with a 2, offline as a LAN guest's.
fn guest_name(player: &str) -> String {
    let base: String = player.chars().take(15).collect();
    format!("{base}2")
}

impl<'a> Game<'a> {
    /// Splitscreen can start: the world is this window's own and on an AirPlay display.
    pub(super) fn splitscreen_available(&self) -> bool {
        !self.splitscreen.guest && self.integrated.is_some() && self.on_airplay()
    }

    /// The second player, joined to this game's integrated server and drawing on the phone's window.
    fn splitscreen_guest(&mut self) -> Result<Game<'a>> {
        let server = self
            .integrated
            .as_ref()
            .ok_or("no world of this window's own")?;
        let window = self.window.clone().ok_or("no window")?;
        let player = horizon_client::auth::current()
            .map_or_else(|| self.options.player_name(), |a| a.name.clone());
        let name = guest_name(&player);
        let pipe = server.join_splitscreen(horizon_protocol::crypto::offline_uuid(&name))?;
        // A client and nothing else: no control socket, no benchmark, no world of its own to open.
        let mut options = self.options.clone();
        options.control = None;
        options.benchmark = None;
        options.frames = None;
        options.world_frames = None;
        options.report = None;
        options.server = None;
        options.world = None;
        options.player_name = Some(name.clone());
        let mut settings = self.settings.clone();
        // The controller is the first player's.
        settings.controller.enabled = false;
        let vanilla = self.vanilla_root();
        let root = super::packs::startup_root(
            &vanilla,
            &self.settings.resource_packs,
            &self.server_packs.active,
        )?;
        let mut atlas = HudAtlas::build_over(&root, &vanilla)?;
        atlas.font.uniform = self.settings.force_unicode;
        atlas.font.japanese = self.settings.japanese_glyphs;
        let gui = UiResources {
            atlas,
            panorama: self.gui.panorama.clone(),
        };
        let audio = Audio::silent(&self.options.resources);
        // The resources outlive both games; the second borrows them as the first does.
        let res: &'a Arc<_> = Box::leak(Box::new(self.res.clone()));
        let mut guest = Game::new(res, self.pack.clone(), gui, options, settings, audio, None)?;
        guest.splitscreen.guest = true;
        guest.ui.vanilla_root = vanilla;
        guest.renderer = Some(guest.new_renderer(window.clone())?);
        guest.world_renderer_ready = true;
        guest.window = Some(window);
        guest.apply_settings();
        guest.previous = Instant::now();
        guest.connect_local(pipe, &name)?;
        log::info!("Horizon: splitscreen: {name} joins on the phone");
        Ok(guest)
    }

    /// The second player has left the world (Disconnect, or the server sent them away).
    fn splitscreen_left(&self) -> bool {
        self.remote.is_none() || self.title.is_some()
    }

    fn end_splitscreen(&mut self) {
        if let Some(server) = &self.integrated {
            server.leave_splitscreen();
        }
        self.splitscreen.active = false;
        log::info!("Horizon: splitscreen ended");
    }
}

/// Starts and ends the second player, once a frame after both games' own.
pub(super) fn update<'a>(host: &mut Game<'a>, guest: &mut Option<Box<Game<'a>>>) {
    let request = host.splitscreen.request.take();
    if guest.is_some()
        && (request == Some(false)
            || guest.as_ref().is_some_and(|g| g.splitscreen_left())
            || host.integrated.is_none()
            || !host.on_airplay())
    {
        // Dropping the guest closes its connection, and the server sees the player leave.
        *guest = None;
        host.end_splitscreen();
    }
    if request == Some(true) && guest.is_none() && host.splitscreen_available() {
        match host.splitscreen_guest() {
            Ok(g) => {
                *guest = Some(Box::new(g));
                host.splitscreen.active = true;
            }
            Err(e) => {
                log::warn!("Horizon: splitscreen: {e}");
                host.end_splitscreen();
            }
        }
    }
}

/// Input from the phone's window: the second player's, while there is one. The rest (redraws, resizes,
/// focus) goes to both games: the first draws on the AirPlay display whenever the window redraws.
pub(super) fn guest_input(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::Touch(_)
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::Ime(_)
            | WindowEvent::ModifiersChanged(_)
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::CursorLeft { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_second_player_is_the_first_with_a_two() {
        assert_eq!(guest_name("Steve"), "Steve2");
        assert_eq!(guest_name("ABCDEFGHIJKLMNOP"), "ABCDEFGHIJKLMNO2");
    }
}
