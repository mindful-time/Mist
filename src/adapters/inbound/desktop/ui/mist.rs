//! Living-mist renderer.

use crate::MistPalette;
use anyhow::{Context, Result};
use eframe::egui::{
    self, Color32, Mesh, Pos2, Rect, Shape, TextureHandle, TextureOptions, Vec2, epaint::Vertex,
};

use super::{
    presentation::{MistActivity, MistPresentation},
    theme::palette_color,
};

pub(super) struct MistRenderer {
    texture: TextureHandle,
}

#[derive(Clone, Copy, Debug)]
struct MistMotion {
    breath: f32,
    turbulence: f32,
}

impl MistRenderer {
    pub(super) fn new(context: &egui::Context) -> Result<Self> {
        let decoded = image::load_from_memory(include_bytes!("../../../../../assets/mist-v2.png"))
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
        let reactivity = audio_reactivity(presentation.features.energy);
        let brightness = f32::from(presentation.features.brightness) / 255.0;
        let motion = motion_for(presentation.activity, reactivity, brightness);
        // Phase is deliberately independent of audio energy. Multiplying the
        // absolute clock by a changing frequency caused visible phase jumps.
        let phase = time * 0.3 + voice_seed;
        let breath_wave = (phase * 0.83).sin();
        let breathing = 1.0 + breath_wave * motion.breath;
        let idle_opacity_breath = if presentation.activity == MistActivity::Idle {
            breath_wave.mul_add(0.5, 0.5) * 6.0
        } else {
            0.0
        };
        let primary = palette_color(palette.primary);
        let secondary = palette_color(palette.secondary);
        let glow = palette_color(palette.glow);

        let span = rect.width().min(rect.height()) * 1.02;
        let layers = [
            (1.0, 0.0, 22u8, primary),
            (0.9, 1.8, 14, secondary),
            (0.78, 3.7, 8, glow),
            (0.67, 5.4, 5, primary),
        ];
        for (index, (scale, offset, alpha, color)) in layers.into_iter().enumerate() {
            let layer_phase = phase * (1.0 + index as f32 * 0.12) + offset;
            // Keep wave frequency constant. Audio only changes the eased
            // amplitude, so brightness updates can never jump wave phase.
            let voice_phase = time * (0.88 + index as f32 * 0.08) + offset;
            let voice_wave = voice_phase.sin();
            let drift = Vec2::new(
                layer_phase.cos() * span * 0.035 * motion.turbulence
                    + voice_wave * span * reactivity * 0.045,
                (layer_phase * 0.73).sin() * span * 0.028 * motion.turbulence
                    + voice_phase.cos() * span * reactivity * 0.032,
            );
            let size = span
                * scale
                * breathing
                * (1.0
                    + reactivity * 0.045
                    + voice_wave * reactivity * 0.105 * (1.0 + index as f32 * 0.11));
            let angle = layer_phase
                * (0.045 + index as f32 * 0.009)
                * if index % 2 == 0 { 1.0 } else { -1.0 }
                + voice_wave * reactivity * (0.035 + index as f32 * 0.006);
            let opacity = (f32::from(alpha) + reactivity * 100.0 + idle_opacity_breath)
                .round()
                .clamp(0.0, 255.0) as u8;
            painter.add(textured_quad(
                self.texture.id(),
                center + drift,
                Vec2::splat(size),
                angle,
                with_alpha(color, opacity),
            ));
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
                features: crate::AudioFeatures {
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

fn audio_reactivity(energy: u8) -> f32 {
    ((f32::from(energy) - 12.0) / 243.0).clamp(0.0, 1.0).sqrt()
}

fn motion_for(activity: MistActivity, reactivity: f32, brightness: f32) -> MistMotion {
    let breath = 0.052 + reactivity * 0.12;
    let color_flow = brightness * 0.12 + reactivity * 0.18;
    match activity {
        MistActivity::Idle => MistMotion {
            breath,
            turbulence: 0.62 + color_flow,
        },
        MistActivity::Busy => MistMotion {
            breath: breath + 0.012,
            turbulence: 0.64 + color_flow,
        },
        MistActivity::Speaking => MistMotion {
            breath,
            turbulence: 0.72 + color_flow,
        },
        MistActivity::Attention => MistMotion {
            breath: breath + 0.008,
            turbulence: 0.58 + color_flow,
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_reactivity_is_quiet_at_idle_and_strong_for_voice() {
        let idle = audio_reactivity(10);
        let speaking = audio_reactivity(220);

        assert!(idle < 0.1);
        assert!(speaking > 0.9);
        assert!(speaking > idle * 10.0);
    }

    #[test]
    fn compact_idle_mist_keeps_a_visible_breath() {
        let idle = motion_for(MistActivity::Idle, 0.0, 16.0 / 255.0);

        assert!(idle.breath >= 0.05);
    }
}
