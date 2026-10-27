//! The game on an AirPlay display. An app that declares the external display scene role
//! (`UIWindowSceneSessionRoleExternalDisplayNonInteractive` in GameInfo.plist) gets that display as a screen
//! of its own instead of a mirror of the phone: iOS connects a scene to [`register`]'s delegate, which puts a
//! window in it, and [`screen`] hands that window's view to the game for wgpu to draw on. The phone, meanwhile,
//! is covered by a view that says "Playing on AirPlay" and takes its touches: on AirPlay the game is played
//! with a controller. The game forbids `unsafe`, so the Objective-C lives here. Elsewhere [`screen`] is always
//! `None`.

use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

/// The AirPlay display, while one is connected: what the game draws to instead of the phone's window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    /// Counts connections, so a display that went and came back is a new surface.
    pub id: u64,
    /// The display's size in pixels.
    pub size: (u32, u32),
    #[cfg(target_os = "ios")]
    view: std::ptr::NonNull<std::ffi::c_void>,
}

// SAFETY: the view is only handed to wgpu, which reads it on the main thread when it makes the surface, and
// `ios::STATE` keeps it alive while the display is connected.
#[cfg(target_os = "ios")]
unsafe impl Send for Screen {}
#[cfg(target_os = "ios")]
unsafe impl Sync for Screen {}

impl HasWindowHandle for Screen {
    #[cfg(target_os = "ios")]
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let handle = raw_window_handle::UiKitWindowHandle::new(self.view);
        // SAFETY: the view is alive for as long as this display is connected (see `Screen`).
        Ok(unsafe { WindowHandle::borrow_raw(handle.into()) })
    }
    #[cfg(not(target_os = "ios"))]
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for Screen {
    #[cfg(target_os = "ios")]
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::uikit())
    }
    #[cfg(not(target_os = "ios"))]
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

/// Make the scene delegate's class known before iOS asks for it by name: call it before the event loop.
pub fn register() {
    #[cfg(target_os = "ios")]
    ios::register();
}

/// The AirPlay display, if one is connected. Called every frame: `cover_phone` says whether the phone shows
/// "Playing on AirPlay" over everything (while the game draws on the display) or what is under it (the
/// launcher, which needs its touches).
pub fn screen(cover_phone: bool) -> Option<Screen> {
    #[cfg(target_os = "ios")]
    return ios::screen(cover_phone);
    #[cfg(not(target_os = "ios"))]
    {
        let _ = cover_phone;
        None
    }
}

#[cfg(target_os = "ios")]
mod ios {
    use super::Screen;
    use objc2::rc::Retained;
    use objc2::{MainThreadMarker, MainThreadOnly, Message, define_class};
    use objc2_core_foundation::CGRect;
    use objc2_foundation::{NSObject, NSObjectProtocol, ns_string};
    use objc2_ui_kit::{
        NSTextAlignment, UIApplication, UIColor, UIFont, UILabel, UIScene,
        UISceneConnectionOptions, UISceneDelegate, UISceneSession, UIView, UIViewAutoresizing,
        UIViewController, UIWindow, UIWindowScene, UIWindowSceneDelegate,
        UIWindowSceneSessionRoleApplication,
    };
    use std::cell::{Cell, RefCell};
    use std::ptr::NonNull;

    struct State {
        id: u64,
        scene: Retained<UIWindowScene>,
        /// Kept: a window nothing retains leaves the display.
        _window: Retained<UIWindow>,
        view: Retained<UIView>,
        cover: Option<Retained<UIView>>,
    }

    thread_local! {
        static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
        static CONNECTIONS: Cell<u64> = const { Cell::new(0) };
    }

    define_class!(
        // The name GameInfo.plist's external display configuration gives.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "HorizonAirPlayDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UISceneDelegate for Delegate {
            #[unsafe(method(scene:willConnectToSession:options:))]
            fn will_connect(
                &self,
                scene: &UIScene,
                _session: &UISceneSession,
                _options: &UISceneConnectionOptions,
            ) {
                connect(scene);
            }

            #[unsafe(method(sceneDidDisconnect:))]
            fn did_disconnect(&self, scene: &UIScene) {
                disconnect(scene);
            }
        }

        unsafe impl UIWindowSceneDelegate for Delegate {}
    );

    pub(super) fn register() {
        let _ = <Delegate as objc2::ClassType>::class();
    }

    fn connect(scene: &UIScene) {
        let Some(scene) = scene.downcast_ref::<UIWindowScene>() else {
            return;
        };
        let mtm = MainThreadMarker::from(scene);
        let window = UIWindow::initWithWindowScene(UIWindow::alloc(mtm), scene);
        let controller = UIViewController::new(mtm);
        let view = controller.view().unwrap_or_else(|| UIView::new(mtm));
        view.setBackgroundColor(Some(&UIColor::blackColor()));
        window.setRootViewController(Some(&controller));
        window.setHidden(false);
        let id = CONNECTIONS.with(|c| {
            c.set(c.get() + 1);
            c.get()
        });
        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                id,
                scene: scene.retain(),
                _window: window,
                view,
                cover: None,
            })
        });
    }

    fn disconnect(scene: &UIScene) {
        STATE.with(|s| {
            let mut state = s.borrow_mut();
            if state.as_ref().is_some_and(|st| {
                std::ptr::eq(&*st.scene as *const UIWindowScene as *const UIScene, scene)
            }) && let Some(st) = state.take()
                && let Some(cover) = st.cover
            {
                cover.removeFromSuperview();
            }
        });
    }

    /// The phone's own window: the one in the scene with the application role.
    fn phone_window(mtm: MainThreadMarker) -> Option<Retained<UIWindow>> {
        let app = UIApplication::sharedApplication(mtm);
        // SAFETY: a UIKit constant, read on the main thread.
        let role = unsafe { UIWindowSceneSessionRoleApplication };
        app.connectedScenes().iter().find_map(|scene| {
            let scene = scene.downcast::<UIWindowScene>().ok()?;
            if *scene.session().role() != *role {
                return None;
            }
            scene.windows().firstObject()
        })
    }

    /// A black view over the phone's window that says where the game is, and takes the touches the hidden
    /// controls would have. Made when first shown: the phone's window may not exist yet when a display that
    /// was already connected joins at launch.
    fn cover_phone(mtm: MainThreadMarker) -> Option<Retained<UIView>> {
        let window = phone_window(mtm)?;
        let bounds: CGRect = window.bounds();
        let flexible = UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight;
        let cover = UIView::initWithFrame(UIView::alloc(mtm), bounds);
        cover.setAutoresizingMask(flexible);
        cover.setBackgroundColor(Some(&UIColor::blackColor()));
        let label = UILabel::initWithFrame(UILabel::alloc(mtm), bounds);
        label.setAutoresizingMask(flexible);
        label.setText(Some(ns_string!("Playing on AirPlay")));
        // SAFETY: a label made here, on the main thread.
        unsafe { label.setTextColor(Some(&UIColor::whiteColor())) };
        label.setTextAlignment(NSTextAlignment::Center);
        unsafe { label.setFont(Some(&UIFont::boldSystemFontOfSize(22.0))) };
        cover.addSubview(&label);
        window.addSubview(&cover);
        Some(cover)
    }

    pub(super) fn screen(cover: bool) -> Option<Screen> {
        STATE.with(|s| {
            let mut state = s.borrow_mut();
            let state = state.as_mut()?;
            if cover && state.cover.is_none() {
                state.cover = cover_phone(MainThreadMarker::from(&*state.view));
            }
            if let Some(view) = &state.cover {
                view.setHidden(!cover);
                // The launcher or the keyboard may have put a view over it since.
                if cover && let Some(window) = view.superview() {
                    window.bringSubviewToFront(view);
                }
            }
            let bounds = state.view.bounds();
            let scale = state.scene.screen().scale();
            let size = (
                (bounds.size.width * scale).round().max(1.0) as u32,
                (bounds.size.height * scale).round().max(1.0) as u32,
            );
            Some(Screen {
                id: state.id,
                size,
                view: NonNull::from(&*state.view).cast(),
            })
        })
    }
}
