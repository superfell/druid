use wayland_client as wlc;
use wayland_protocols::unstable::xdg_decoration::v1::client::zxdg_toplevel_decoration_v1 as toplevel_decorations;
use wayland_protocols::xdg_shell::client::xdg_surface;
use wayland_protocols::xdg_shell::client::xdg_toplevel;

use crate::kurbo;
use crate::window;

use super::surface;
use super::Compositor;
use super::CompositorHandle;
use super::Decor;
use super::Handle;

struct Inner {
    wl_surface: surface::Handle,
    #[allow(unused)]
    pub(super) xdg_surface: wlc::Main<xdg_surface::XdgSurface>,
    pub(super) xdg_toplevel: wlc::Main<xdg_toplevel::XdgToplevel>,
    #[allow(unused)]
    pub(super) zxdg_toplevel_decoration_v1:
        wlc::Main<toplevel_decorations::ZxdgToplevelDecorationV1>,
}

impl From<Inner> for u32 {
    fn from(s: Inner) -> u32 {
        u32::from(s.wl_surface)
    }
}

impl From<Inner> for std::sync::Arc<surface::Data> {
    fn from(s: Inner) -> std::sync::Arc<surface::Data> {
        std::sync::Arc::<surface::Data>::from(s.wl_surface)
    }
}

#[derive(Clone)]
pub struct Surface {
    inner: std::sync::Arc<Inner>,
}

impl Surface {
    pub fn new(
        c: impl Into<CompositorHandle>,
        handler: Box<dyn window::WinHandler>,
        initial_size: kurbo::Size,
        min_size: Option<kurbo::Size>,
    ) -> Self {
        let compositor = CompositorHandle::new(c);
        let wl_surface = surface::Handle::new(compositor.clone(), handler, kurbo::Size::ZERO);
        let xdg_surface = compositor.get_xdg_surface(&wl_surface.inner.wl_surface);
        let xdg_toplevel = xdg_surface.get_toplevel();
        let zxdg_toplevel_decoration_v1 = compositor
            .zxdg_decoration_manager_v1()
            .get_toplevel_decoration(&xdg_toplevel);

        // register to receive xdg_surface events.
        xdg_surface.quick_assign({
            let wl_surface = wl_surface.clone();
            move |xdg_surface, event, _| {
                tracing::trace!("xdg_surface event configure {:?}", event);
                match event {
                    xdg_surface::Event::Configure { serial } => {
                        xdg_surface.ack_configure(serial);
                        let dim = wl_surface.inner.logical_size.get();
                        wl_surface.inner.handler.borrow_mut().size(dim);
                        wl_surface.inner.buffers.request_paint();
                    }
                    _ => tracing::warn!("unhandled xdg_surface event {:?}", event),
                }
            }
        });

        xdg_toplevel.quick_assign({
            let wl_surface = wl_surface.clone();
            let mut dim = initial_size.clone();
            move |_xdg_toplevel, event, a3| match event {
                xdg_toplevel::Event::Configure {
                    width,
                    height,
                    states,
                } => {
                    tracing::trace!(
                        "configure event {:?} {:?} {:?} {:?}",
                        width,
                        height,
                        states,
                        a3
                    );
                    // compositor is deferring to the client for determining the size
                    // when values are zero.
                    if width != 0 && height != 0 {
                        dim = kurbo::Size::new(width as f64, height as f64);
                    }
                    wl_surface.update_dimensions(dim.width as u32, dim.height as u32);
                }
                xdg_toplevel::Event::Close => {
                    tracing::info!("xdg close event {:?}", event);
                    wl_surface.inner.handler.borrow_mut().request_close();
                }
                _ => tracing::info!("unimplemented event {:?}", event),
            }
        });

        zxdg_toplevel_decoration_v1.quick_assign(move |_zxdg_toplevel_decoration_v1, event, _| {
            match event {
                _ => tracing::info!("toplevel decoration unimplemented {:?}", event),
            }
        });

        let inner = Inner {
            wl_surface,
            xdg_toplevel,
            xdg_surface,
            zxdg_toplevel_decoration_v1,
        };

        inner
            .zxdg_toplevel_decoration_v1
            .set_mode(toplevel_decorations::Mode::ServerSide);
        if let Some(size) = min_size {
            inner
                .xdg_toplevel
                .set_min_size(size.width as i32, size.height as i32);
        }

        let handle = Self {
            inner: std::sync::Arc::new(inner),
        };

        handle.commit();
        handle
    }

    pub(crate) fn with_handler<T, F: FnOnce(&mut dyn window::WinHandler) -> T>(
        &self,
        f: F,
    ) -> Option<T> {
        std::sync::Arc::<surface::Data>::from(self).with_handler(f)
    }

    pub(crate) fn commit(&self) {
        self.inner.wl_surface.commit();
    }
}

impl Decor for Surface {
    fn inner_set_title(&self, title: String) {
        self.inner.xdg_toplevel.set_title(title);
    }
}

impl From<Surface> for u32 {
    fn from(s: Surface) -> u32 {
        u32::from(s.inner.wl_surface.clone())
    }
}

impl From<&Surface> for u32 {
    fn from(s: &Surface) -> u32 {
        u32::from(s.inner.wl_surface.clone())
    }
}

impl From<&Surface> for std::sync::Arc<surface::Data> {
    fn from(s: &Surface) -> std::sync::Arc<surface::Data> {
        std::sync::Arc::<surface::Data>::from(s.inner.wl_surface.clone())
    }
}

impl From<Surface> for std::sync::Arc<surface::Data> {
    fn from(s: Surface) -> std::sync::Arc<surface::Data> {
        std::sync::Arc::<surface::Data>::from(s.inner.wl_surface.clone())
    }
}

impl From<Surface> for Box<dyn Handle> {
    fn from(s: Surface) -> Box<dyn Handle> {
        Box::new(s.inner.wl_surface.clone()) as Box<dyn Handle>
    }
}

impl From<Surface> for Box<dyn Decor> {
    fn from(s: Surface) -> Box<dyn Decor> {
        Box::new(s.clone()) as Box<dyn Decor>
    }
}