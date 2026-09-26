use anyhow::{Context, Result};
use eframe::egui::{
    self, Color32, Mesh, Pos2, Rect, Shape, TextureHandle, TextureOptions, Vec2, epaint::Vertex,
};
use select_to_speak::MistPalette;

use super::{
    presentation::{MistActivity, MistPresentation},
    theme::palette_color,
};

pub(super) struct MistRenderer {
    texture: TextureHandle,
}

impl MistRenderer {
    pub(super) fn new(context: &egui::Context) -> Result<Self> {
        let decoded = image::load_from_memory(include_bytes!("../../assets/mist.png"))
            .context("the embedded mist texture is invalid")?
            .into_rgba8();
        let size = [decoded.width() as usize, decoded.height() as usize];
        let image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
        let texture = context.load_texture("living-mist", image, TextureOptions::LINEAR);
        Ok(Self { texture })
    }

    pub(super) fn paint(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        time: f32,
        presentation: MistPresentation,
        palette: MistPalette,
        voice_seed: f32,
    ) {
        let painter = ui.painter();
        let center = rect.center();
        let energy = (f32::from(presentation.features.energy) / 255.0).sqrt();
        let brightness = f32::from(presentation.features.brightness) / 255.0;
        let (speed, breath, turbulence) = match presentation.activity {
            MistActivity::Idle => (0.16, 0.035, 0.35),
            MistActivity::Busy => (0.55, 0.055, 0.75),
            MistActivity::Speaking => (
                1.2 + energy * 1.7 + brightness * 0.7,
                0.08 + energy * 0.16,
                0.8 + brightness * 0.7,
            ),
            MistActivity::Attention => (0.38, 0.06, 0.55),
        };
        let phase = time * speed + voice_seed;
        let breathing = 1.0 + phase.sin() * breath;
        let primary = palette_color(palette.primary);
        let secondary = palette_color(palette.secondary);
        let glow = palette_color(palette.glow);

        for (radius, alpha) in [(0.43, 9), (0.33, 12), (0.22, 16)] {
            painter.circle_filled(
                center,
                rect.width().min(rect.height()) * radius * (1.0 + energy * 0.08),
                with_alpha(glow, alpha),
            );
        }

        let span = rect.width().min(rect.height()) * 0.88;
        let layers = [
            (1.03, 0.0, 58, primary),
            (0.91, 1.8, 44, secondary),
            (0.76, 3.7, 36, glow),
            (0.63, 5.4, 29, primary),
        ];
        for (index, (scale, offset, alpha, color)) in layers.into_iter().enumerate() {
            let layer_phase = phase * (1.0 + index as f32 * 0.12) + offset;
            let drift = Vec2::new(
                layer_phase.cos() * span * 0.035 * turbulence,
                (layer_phase * 0.73).sin() * span * 0.028 * turbulence,
            );
            let voice_wave = (phase * (1.4 + voice_seed * 0.03) + offset).sin();
            let size = span
                * scale
                * breathing
                * (1.0 + voice_wave * energy * (0.035 + index as f32 * 0.008));
            let angle = layer_phase
                * (0.055 + index as f32 * 0.012)
                * if index % 2 == 0 { 1.0 } else { -1.0 };
            let opacity = alpha + (energy * 42.0) as u8;
            painter.add(textured_quad(
                self.texture.id(),
                center + drift,
                Vec2::splat(size),
                angle,
                with_alpha(color, opacity),
            ));
        }

        if presentation.activity == MistActivity::Speaking {
            let pulse = (phase * 2.6).sin().mul_add(0.5, 0.5);
            painter.circle_stroke(
                center,
                span * (0.28 + energy * 0.1 + pulse * 0.018),
                egui::Stroke::new(0.7, with_alpha(glow, (18.0 + energy * 45.0) as u8)),
            );
        }
    }

    pub(super) fn paint_swatch(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        time: f32,
        palette: MistPalette,
        seed: f32,
    ) {
        self.paint(
            ui,
            rect,
            time,
            MistPresentation {
                activity: MistActivity::Idle,
                features: select_to_speak::AudioFeatures {
                    energy: 12,
                    brightness: 20,
                },
                requires_panel: false,
            },
            palette,
            seed,
        );
    }
}

fn textured_quad(
    texture: egui::TextureId,
    center: Pos2,
    size: Vec2,
    angle: f32,
    color: Color32,
) -> Shape {
    let sin = angle.sin();
    let cos = angle.cos();
    let rotate =
        |point: Vec2| Vec2::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos);
    let half = size * 0.5;
    let points = [
        (-half.x, -half.y, 0.0, 0.0),
        (half.x, -half.y, 1.0, 0.0),
        (half.x, half.y, 1.0, 1.0),
        (-half.x, half.y, 0.0, 1.0),
    ];
    let mut mesh = Mesh::with_texture(texture);
    for (x, y, u, v) in points {
        mesh.vertices.push(Vertex {
            pos: center + rotate(Vec2::new(x, y)),
            uv: Pos2::new(u, v),
            color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    Shape::mesh(mesh)
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}
