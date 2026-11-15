//! Screenshots of the last frame through egui's off-screen wgpu renderer
//! (the one the window's render tests use). Without a GPU adapter the
//! renderer cannot be made, and the tool says so instead of failing hard.

use egui_kittest::wgpu::{default_wgpu_setup, WgpuTestRenderer};
use egui_kittest::TestRenderer;
use geo_app::headless::egui::{Context, FullOutput, TexturesDelta};

pub struct Renderer {
    inner: WgpuTestRenderer,
}

impl Renderer {
    /// Makes the renderer, or tells why there is none (no adapter).
    pub fn new() -> Result<Renderer, String> {
        let setup = default_wgpu_setup();
        let instance = pollster::block_on(setup.new_instance());
        let config = egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup,
            ..Default::default()
        };
        let state = pollster::block_on(egui_wgpu::RenderState::create(
            &config,
            &instance,
            None,
            egui_wgpu::RendererOptions::PREDICTABLE,
        ))
        .map_err(|e| format!("screenshots need a GPU adapter, and none is usable here: {e}"))?;
        Ok(Renderer {
            inner: WgpuTestRenderer::from_render_state(state),
        })
    }

    /// Renders `out` (a frame of `ctx`) after applying `textures`, and
    /// writes it to `path` as a PNG. Returns the image size in pixels.
    pub fn render(
        &mut self,
        ctx: &Context,
        out: &FullOutput,
        textures: &mut TexturesDelta,
        path: &str,
    ) -> Result<(u32, u32), String> {
        self.inner.handle_delta(textures);
        let image = self.inner.render(ctx, out)?;
        image
            .save_with_format(path, image::ImageFormat::Png)
            .map_err(|e| format!("{path}: {e}"))?;
        Ok(image.dimensions())
    }
}
