//! Living-mist renderer.

use std::f32::consts::TAU;

use crate::MistPalette;
use anyhow::{Context, Result};
use eframe::egui::{
    self, Color32, Mesh, Pos2, Rect, Shape, TextureHandle, TextureOptions, Vec2, epaint::Vertex,
};

use super::{
    presentation::{MistActivity, MistPresentation},
    theme::palette_color,
};

const DEPTH_BANDS: [DepthBand; 4] = [
    DepthBand::new(0.48, 0.078, 0.72, 5.4),
    DepthBand::new(0.62, 0.096, 0.82, 3.7),
    DepthBand::new(0.80, 0.116, 0.92, 1.8),
    DepthBand::new(1.00, 0.142, 1.00, 0.0),
];
const WISP_UVS: [[f32; 4]; 3] = [
    [0.00, 0.22, 0.48, 0.82],
    [0.27, 0.26, 0.75, 0.80],
    [0.52, 0.20, 1.00, 0.84],
];
const MIN_WISPS: usize = 4;
const MAX_WISPS: usize = 28;
const WISP_AREA: f32 = 1_350.0;
const ORB_RINGS: [f32; 7] = [0.18, 0.36, 0.54, 0.70, 0.82, 0.92, 1.0];
const ORB_SEGMENTS: usize = 24;
const ORB_VERTICES: usize = 1 + ORB_RINGS.len() * ORB_SEGMENTS;

pub(super) struct MistRenderer {
    volume: TextureHandle,
    wisps: TextureHandle,
}

#[derive(Clone, Copy, Debug)]
struct MistMotion {
    breath: f32,
    turbulence: f32,
}

#[derive(Clone, Copy, Debug)]
struct DepthBand {
    depth: f32,
    opacity: f32,
    scale: f32,
    phase: f32,
}

impl DepthBand {
    const fn new(depth: f32, opacity: f32, scale: f32, phase: f32) -> Self {
        Self {
            depth,
            opacity,
            scale,
            phase,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WispSeed {
    band: usize,
    start: f32,
    anchor_y: f32,
    speed: f32,
    lift: f32,
    size: f32,
    aspect: f32,
    phase: f32,
    secondary_phase: f32,
    breath_offset: f32,
    opacity: f32,
    sprite: usize,
    mirrored: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WispPose {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    angle: f32,
    opacity: f32,
}

#[derive(Clone, Copy)]
struct PaintScene {
    rect: Rect,
    time: f32,
    presentation: MistPresentation,
    palette: MistPalette,
    voice_seed: f32,
}

impl MistRenderer {
    pub(super) fn new(context: &egui::Context) -> Result<Self> {
        Ok(Self {
            volume: load_texture(
                context,
                "mist-orb-volume",
                include_bytes!("../../../../../assets/mist-orb-v1.png"),
            )?,
            wisps: load_texture(
                context,
                "mist-orb-wisps",
                include_bytes!("../../../../../assets/mist-field-alpha-v1.webp"),
            )?,
        })
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
        let scene = PaintScene {
            rect,
            time,
            presentation,
            palette,
            voice_seed,
        };
        self.paint_volume(ui, scene);
        self.paint_wisps(ui, scene);
    }

    fn paint_volume(&self, ui: &egui::Ui, scene: PaintScene) {
        let reactivity = audio_reactivity(scene.presentation.features.energy);
        let breath = 1.0 + (scene.time * 0.249 + scene.voice_seed).sin() * 0.035;
        let scale = breath + reactivity * 0.055;
        let angle = scene.voice_seed * 0.31 + scene.time * 0.004;
        let color = with_alpha(
            palette_color(scene.palette.glow),
            ((0.78 + reactivity * 0.14) * 255.0).round() as u8,
        );
        let mut mesh = Mesh::with_texture(self.volume.id());
        append_circular_layer(&mut mesh, scene.rect, |point| {
            let point = rotate(point, -angle) / scale;
            (Pos2::new(0.5 + point.x * 0.5, 0.5 + point.y * 0.5), color)
        });
        ui.painter().add(Shape::mesh(mesh));
    }

    fn paint_wisps(&self, ui: &egui::Ui, scene: PaintScene) {
        let reactivity = audio_reactivity(scene.presentation.features.energy);
        let brightness = f32::from(scene.presentation.features.brightness) / 255.0;
        let motion = motion_for(scene.presentation.activity, reactivity, brightness);
        let colors = [
            palette_color(scene.palette.glow),
            palette_color(scene.palette.secondary),
            palette_color(scene.palette.primary),
        ];
        let count = wisp_count(scene.rect);
        let mut mesh = Mesh::with_texture(self.wisps.id());
        mesh.vertices.reserve(count * ORB_VERTICES);
        mesh.indices
            .reserve(count * ORB_SEGMENTS * (ORB_RINGS.len() * 6 - 3));

        // Each particle keeps its own phase, crop and velocity. All geometry
        // belongs to the same feathered disk, including rectangular swatches.
        for index in 0..count {
            let seed = wisp_seed(index, scene.voice_seed);
            let pose = wisp_pose(seed, scene.time, motion, reactivity);
            let color = colors[(index + seed.band) % colors.len()];
            let color = with_alpha(color, (pose.opacity * 255.0).round() as u8);
            append_circular_layer(&mut mesh, scene.rect, |point| {
                wisp_sample(point, pose, color, WISP_UVS[seed.sprite], seed.mirrored)
            });
        }

        ui.painter().add(Shape::mesh(mesh));
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

fn load_texture(context: &egui::Context, name: &str, bytes: &[u8]) -> Result<TextureHandle> {
    let decoded = image::load_from_memory(bytes)
        .with_context(|| format!("the embedded {name} texture is invalid"))?
        .into_rgba8();
    let size = [decoded.width() as usize, decoded.height() as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, decoded.as_raw());
    Ok(context.load_texture(name, image, TextureOptions::LINEAR))
}

fn wisp_count(rect: Rect) -> usize {
    let diameter = rect.width().min(rect.height());
    ((diameter * diameter / WISP_AREA).round() as usize).clamp(MIN_WISPS, MAX_WISPS)
}

fn wisp_seed(index: usize, voice_seed: f32) -> WispSeed {
    let voice_key = voice_seed
        .to_bits()
        .rotate_left(11)
        .wrapping_mul(0x9e37_79b9);
    let variation = |salt| seeded_unit(index, salt ^ voice_key);
    let band = (index + voice_key as usize) % DEPTH_BANDS.len();
    let depth = DEPTH_BANDS[band];
    WispSeed {
        band,
        start: variation(0x9e37_79b9),
        anchor_y: 0.22 + variation(0x85eb_ca6b) * 0.56,
        speed: (0.018 + variation(0xc2b2_ae35) * 0.032) * (0.82 + depth.depth * 0.24),
        lift: 0.018 + variation(0x27d4_eb2f) * 0.044,
        size: (0.26 + variation(0x1656_67b1) * 0.22) * depth.scale,
        aspect: 0.48 + variation(0xd3a2_646c) * 0.28,
        phase: variation(0xfd70_46c5) * TAU + depth.phase + voice_seed * 0.19,
        secondary_phase: variation(0xb55a_4f09) * TAU + voice_seed * 0.11,
        breath_offset: (variation(0x94d0_49bb) - 0.5) * 0.9,
        opacity: depth.opacity * (0.78 + variation(0xed5a_d4bb) * 0.46),
        sprite: (index + voice_key as usize) % WISP_UVS.len(),
        mirrored: (index + voice_key as usize) % 2 == 1,
    }
}

fn seeded_unit(index: usize, salt: u32) -> f32 {
    let mut value = (index as u32).wrapping_add(1).wrapping_mul(salt);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    (value & 0x00ff_ffff) as f32 / 0x0100_0000 as f32
}

fn wisp_pose(seed: WispSeed, time: f32, motion: MistMotion, reactivity: f32) -> WispPose {
    let band = DEPTH_BANDS[seed.band];
    let progress = (seed.start + time * seed.speed).rem_euclid(1.0);
    let x = -0.18 + progress * 1.36;
    let coherent_curl = (x * TAU * 1.35 + time * 0.24 + band.phase).sin();
    let local_curl = ((x * 1.9 + seed.anchor_y * 0.8) * TAU - time * 0.17 + seed.phase).cos();
    let curl = (coherent_curl * 0.034 + local_curl * 0.022) * band.depth * motion.turbulence;
    let y = seed.anchor_y + curl - seed.lift * progress;
    let breath = 1.0
        + (time * 0.249 + seed.breath_offset).sin() * (motion.breath * (0.54 + band.depth * 0.46));
    let width = seed.size * breath * (1.0 + reactivity * 0.16);
    let height = width * seed.aspect;
    let edge_fade = horizontal_fade(x, seed.size);
    let vertical_fade = smoothstep(0.06, 0.22, y) * (1.0 - smoothstep(0.78, 0.94, y));
    let shimmer = 0.84 + (time * 0.3 + seed.secondary_phase).sin() * 0.16;
    let opacity = seed.opacity * (1.0 + reactivity * 1.55) * edge_fade * vertical_fade * shimmer;
    let angle = (coherent_curl * 0.09 + local_curl * 0.045).clamp(-0.18, 0.18);

    WispPose {
        x,
        y,
        width,
        height,
        angle,
        opacity,
    }
}

fn horizontal_fade(x: f32, size: f32) -> f32 {
    smoothstep(-size, 0.10, x) * (1.0 - smoothstep(0.82, 1.0 + size, x))
}

fn smoothstep(edge_start: f32, edge_end: f32, value: f32) -> f32 {
    let progress = ((value - edge_start) / (edge_end - edge_start)).clamp(0.0, 1.0);
    progress * progress * (3.0 - 2.0 * progress)
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

fn wisp_sample(
    point: Vec2,
    pose: WispPose,
    color: Color32,
    uv: [f32; 4],
    mirrored: bool,
) -> (Pos2, Color32) {
    let offset = point * 0.5 + Vec2::splat(0.5) - Vec2::new(pose.x, pose.y);
    let local = rotate(offset, -pose.angle) / Vec2::new(pose.width, pose.height);
    let alpha = 1.0 - smoothstep(0.56, 1.0, (local * 2.0).length());
    let x = (local.x + 0.5).clamp(0.0, 1.0);
    let y = (local.y + 0.5).clamp(0.0, 1.0);
    let (u_left, u_right) = if mirrored {
        (uv[2], uv[0])
    } else {
        (uv[0], uv[2])
    };
    (
        Pos2::new(u_left + (u_right - u_left) * x, uv[1] + (uv[3] - uv[1]) * y),
        scale_alpha(color, alpha),
    )
}

fn rotate(point: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

fn append_circular_layer(mesh: &mut Mesh, rect: Rect, sample: impl Fn(Vec2) -> (Pos2, Color32)) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.48;
    let base = mesh.vertices.len() as u32;
    let (uv, color) = sample(Vec2::ZERO);
    mesh.vertices.push(Vertex {
        pos: center,
        uv,
        color,
    });

    for ring in ORB_RINGS {
        let edge_alpha = 1.0 - smoothstep(0.78, 1.0, ring);
        for segment in 0..ORB_SEGMENTS {
            let angle = segment as f32 * TAU / ORB_SEGMENTS as f32;
            let point = Vec2::new(angle.cos(), angle.sin()) * ring;
            let (uv, color) = sample(point);
            mesh.vertices.push(Vertex {
                pos: center + point * radius,
                uv,
                color: scale_alpha(color, edge_alpha),
            });
        }
    }

    for segment in 0..ORB_SEGMENTS {
        let current = segment as u32;
        let next = ((segment + 1) % ORB_SEGMENTS) as u32;
        mesh.indices
            .extend_from_slice(&[base, base + 1 + current, base + 1 + next]);
        for ring in 0..ORB_RINGS.len() - 1 {
            let inner = base + 1 + (ring * ORB_SEGMENTS) as u32;
            let outer = inner + ORB_SEGMENTS as u32;
            mesh.indices.extend_from_slice(&[
                inner + current,
                outer + current,
                outer + next,
                inner + current,
                outer + next,
                inner + next,
            ]);
        }
    }
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn scale_alpha(color: Color32, factor: f32) -> Color32 {
    color.gamma_multiply(factor)
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

    #[test]
    fn seeded_wisps_flow_right_independently() {
        let first = wisp_seed(0, 0.0);
        let second = wisp_seed(1, 0.0);
        let motion = motion_for(MistActivity::Idle, 0.0, 0.0);
        let first_x = wisp_pose(first, 2.0, motion, 0.0).x;
        let first_later = wisp_pose(first, 2.01, motion, 0.0).x;

        assert!(first.speed > 0.0);
        assert!(second.speed > 0.0);
        assert_ne!(first.speed, second.speed);
        assert_ne!(first.phase, second.phase);
        assert!(first_later > first_x);
    }

    #[test]
    fn each_voice_has_stable_distinct_texture_and_motion() {
        let heart = wisp_seed(3, 0.0);
        let bella = wisp_seed(3, 0.83);

        assert_eq!(heart, wisp_seed(3, 0.0));
        assert_eq!(bella, wisp_seed(3, 0.83));
        assert_ne!(heart.start, bella.start);
        assert_ne!(heart.speed, bella.speed);
        assert_ne!(heart.sprite, bella.sprite);
        assert_ne!(heart.phase, bella.phase);
    }

    #[test]
    fn wisps_recycle_with_invisible_edges() {
        let size = 0.3;

        assert_eq!(horizontal_fade(-size, size), 0.0);
        assert!(horizontal_fade(0.5, size) > 0.9);
        assert_eq!(horizontal_fade(1.0 + size, size), 0.0);

        let before_wrap = -0.18 + 0.99_f32.rem_euclid(1.0) * 1.36;
        let after_wrap = -0.18 + 1.01_f32.rem_euclid(1.0) * 1.36;
        assert!(after_wrap < before_wrap);
    }

    #[test]
    fn surface_size_bounds_particle_work() {
        let swatch = Rect::from_min_size(Pos2::ZERO, Vec2::splat(52.0));
        let rectangular_swatch = Rect::from_min_size(Pos2::ZERO, Vec2::new(104.0, 52.0));
        let idle = Rect::from_min_size(Pos2::ZERO, super::super::MIST_WINDOW);
        let speaking = Rect::from_min_size(Pos2::ZERO, super::super::SPEAKING_MIST_WINDOW);

        assert_eq!(wisp_count(swatch), MIN_WISPS);
        assert_eq!(wisp_count(rectangular_swatch), MIN_WISPS);
        assert_eq!(wisp_count(idle), 20);
        assert_eq!(wisp_count(speaking), MAX_WISPS);
    }

    #[test]
    fn each_wisp_feathers_away_before_its_texture_edges() {
        let pose = WispPose {
            x: 0.5,
            y: 0.5,
            width: 0.4,
            height: 0.2,
            angle: 0.0,
            opacity: 1.0,
        };
        let color = Color32::from_white_alpha(120);
        let (_, center) = wisp_sample(Vec2::ZERO, pose, color, WISP_UVS[0], false);
        let (_, feather) = wisp_sample(Vec2::new(0.32, 0.0), pose, color, WISP_UVS[0], false);
        let (_, edge) = wisp_sample(Vec2::new(0.4, 0.2), pose, color, WISP_UVS[0], true);

        assert_eq!(center.a(), 120);
        assert!(feather.a() > 0 && feather.a() < center.a());
        assert_eq!(edge.a(), 0);
    }

    fn scene(size: Vec2) -> PaintScene {
        PaintScene {
            rect: Rect::from_min_size(Pos2::new(20.0, 20.0), size),
            time: 2.0,
            presentation: MistPresentation {
                activity: MistActivity::Idle,
                features: crate::AudioFeatures {
                    energy: 12,
                    brightness: 20,
                },
                requires_panel: false,
            },
            palette: MistPalette {
                primary: [255, 130, 173],
                secondary: [247, 182, 226],
                glow: [255, 224, 239],
            },
            voice_seed: 0.0,
        }
    }

    fn painted_meshes(scene: PaintScene, swatch: bool) -> Vec<Mesh> {
        let context = egui::Context::default();
        let renderer = MistRenderer::new(&context).unwrap();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                ..Default::default()
            },
            |ui| {
                if swatch {
                    renderer.paint_swatch(
                        ui,
                        scene.rect,
                        scene.time,
                        scene.palette,
                        scene.voice_seed,
                    );
                } else {
                    renderer.paint(
                        ui,
                        scene.rect,
                        scene.time,
                        scene.presentation,
                        scene.palette,
                        scene.voice_seed,
                    );
                }
            },
        );
        let meshes = std::mem::take(&mut output.shapes)
            .into_iter()
            .filter_map(|shape| match shape.shape {
                Shape::Mesh(mesh) => Some((*mesh).clone()),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        meshes
    }

    #[test]
    fn actual_idle_and_speaking_paint_produce_visible_circular_meshes() {
        for (activity, energy, size) in [
            (MistActivity::Idle, 12, super::super::MIST_WINDOW),
            (
                MistActivity::Speaking,
                220,
                super::super::SPEAKING_MIST_WINDOW,
            ),
            (MistActivity::Busy, 55, Vec2::new(94.0, 52.0)),
            (MistActivity::Attention, 12, Vec2::splat(142.0)),
        ] {
            let mut scene = scene(size);
            scene.presentation.activity = activity;
            scene.presentation.features.energy = energy;
            let meshes = painted_meshes(scene, false);

            assert_eq!(meshes.len(), 2);
            assert_ne!(meshes[0].texture_id, meshes[1].texture_id);
            assert!(meshes[0].vertices[0].color.a() >= 190);
            assert!(meshes[1].vertices.iter().any(|vertex| vertex.color.a() > 0));
            assert_eq!(
                meshes[1].vertices.len(),
                wisp_count(scene.rect) * ORB_VERTICES
            );
            for mesh in meshes {
                assert!(mesh.is_valid());
                for layer in mesh.vertices.chunks_exact(ORB_VERTICES) {
                    for vertex in layer {
                        assert!(
                            vertex.pos.distance(scene.rect.center())
                                <= size.x.min(size.y) * 0.48 + 0.001
                        );
                    }
                    assert!(
                        layer[ORB_VERTICES - ORB_SEGMENTS..]
                            .iter()
                            .all(|vertex| vertex.color.a() == 0)
                    );
                }
            }
        }
    }

    #[test]
    fn swatches_keep_stable_voice_crops_palette_and_independent_motion() {
        let first_scene = scene(Vec2::new(94.0, 52.0));
        let first = painted_meshes(first_scene, true);
        assert_eq!(first, painted_meshes(first_scene, true));

        let mut second_scene = first_scene;
        second_scene.voice_seed = 0.83;
        let second = painted_meshes(second_scene, true);
        assert_ne!(first[0].vertices[1].uv, second[0].vertices[1].uv);
        assert_ne!(first[1].vertices, second[1].vertices);

        second_scene = first_scene;
        second_scene.palette.glow = [202, 226, 255];
        assert_ne!(
            first[0].vertices[0].color,
            painted_meshes(second_scene, true)[0].vertices[0].color
        );

        second_scene = first_scene;
        second_scene.time += 1.0;
        let later = painted_meshes(second_scene, true);
        assert_ne!(first[1].vertices, later[1].vertices);
        let motion = motion_for(MistActivity::Idle, 0.0, 0.0);
        let travel = |index| {
            let seed = wisp_seed(index, first_scene.voice_seed);
            wisp_pose(seed, second_scene.time, motion, 0.0).x
                - wisp_pose(seed, first_scene.time, motion, 0.0).x
        };
        assert_ne!(travel(0), travel(1));
        assert_eq!(first[1].vertices.len(), MIN_WISPS * ORB_VERTICES);
    }
}
