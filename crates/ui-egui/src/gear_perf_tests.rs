//! Per-frame time of the Gear Generator window, by type, turning and still, on the CPU
//! rasteriser and (with an adapter) the GPU, with the 3D view's share. Not a pass/fail check:
//! run with `--ignored --nocapture`.

use crate::gear_view3d::perf;
use crate::gear_window::RightView;
use solidcraft_gear::plan::Kind;

fn summary(h: &mut crate::scenario::Harness, frames: usize, gpu: Option<&crate::gear_view3d_gpu::Shared>) -> String {
    let mut total = Vec::new();
    let mut parts: std::collections::BTreeMap<&'static str, f64> = Default::default();
    let mut tess = 0.0;
    perf::take();
    for _ in 0..frames {
        let t = std::time::Instant::now();
        h.frame();
        if let Some(g) = gpu {
            g.wait();
        }
        total.push(t.elapsed().as_secs_f64() * 1e3);
        let t = std::time::Instant::now();
        let shapes = std::mem::take(&mut h.shapes);
        let _ = h.ctx.tessellate(shapes, 1.0);
        tess += t.elapsed().as_secs_f64() * 1e3;
        for (n, ms) in perf::take() {
            *parts.entry(n).or_default() += ms;
        }
    }
    total.sort_by(f64::total_cmp);
    let mean = total.iter().sum::<f64>() / frames as f64;
    let p90 = total[(frames * 9 / 10).min(frames - 1)];
    let mut s = format!("frame mean {mean:6.1} p90 {p90:6.1} | tess {:5.1}", tess / frames as f64);
    for (n, ms) in parts {
        s += &format!(" | {n} {:5.1}", ms / frames as f64);
    }
    s
}

#[test]
#[ignore]
fn gear_window_frame_times() {
    let shared = crate::gpu::offscreen_device().map(|(_, device, queue)| {
        let renderer = std::sync::Arc::new(egui::epaint::mutex::RwLock::new(egui_wgpu::Renderer::new(&device, egui_wgpu::wgpu::TextureFormat::Rgba8UnormSrgb, Default::default())));
        crate::gear_view3d_gpu::Shared::new(device, queue, renderer)
    });
    let mut h = crate::scenario::Harness::new();
    perf::take();
    eprintln!("no window: {}", summary(&mut h, 10, None));
    crate::gear_window::open(&mut h.app);
    h.frames(5);
    for gpu in [false, true] {
        if gpu && shared.is_none() {
            eprintln!("no GPU adapter: GPU pass skipped");
            continue;
        }
        crate::gear_view3d_gpu::set_shared(if gpu { shared.clone() } else { None });
        let g = if gpu { shared.as_deref() } else { None };
        for (right, label) in [(RightView::Both, "both"), (RightView::View3d, "3d")] {
            for k in [Kind::Pair, Kind::Helical, Kind::SpiralBevel, Kind::Hypoid, Kind::Worm, Kind::BevelPlanetary, Kind::Planetary, Kind::Differential] {
                {
                    let w = h.app.gear_window.as_mut().unwrap();
                    crate::gear_simple::set_mode(w, k);
                    w.right = right;
                    w.paused = false;
                }
                h.frames(5);
                std::thread::sleep(std::time::Duration::from_millis(300));
                h.frames(5);
                let tris = h.app.gear_window.as_ref().unwrap().view3d.cost().map_or(0, |c| c.0);
                let on = if gpu { "gpu" } else { "cpu" };
                eprintln!("{on} {label} {k:?} ({}k tris) turning: {}", tris / 1000, summary(&mut h, 20, g));
                h.app.gear_window.as_mut().unwrap().paused = true;
                h.frames(2);
                eprintln!("{on} {label} {k:?} still:   {}", summary(&mut h, 10, g));
            }
        }
    }
    crate::gear_view3d_gpu::set_shared(None);
}
