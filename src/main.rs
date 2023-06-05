use std::fmt::Debug;
use std::fs;

use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::{format_err, Context};
use log::debug;
use tiny_skia::Pixmap;

use crate::document::Document;
use crate::evdev::KeyDeviceBuilder;
use crate::framebuffer::Framebuffer;
use crate::rendering::Renderer;
use crate::scripting::Script;
use crate::sleep::Sleeper;
use crate::sleep::WakeupReason;
use crate::timer::Timer;

mod document;
mod evdev;
mod framebuffer;
mod opts;
mod rendering;
mod scripting;
mod sleep;
mod timer;

#[derive(Debug)]
enum Canvas {
    Framebuffer(Framebuffer),
    Image(PathBuf),
}

impl Canvas {
    fn from_opts(ouput: opts::Output) -> Result<Self, anyhow::Error> {
        Ok(match ouput {
            opts::Output::Framebuffer {
                device,
                eink_refresh_rate,
            } => {
                let fb = framebuffer::Builder::with_device(device)
                    .eink_refresh_rate(eink_refresh_rate)
                    .open()?;
                Canvas::Framebuffer(fb)
            }
            opts::Output::Image(path) => Canvas::Image(path),
        })
    }

    fn screen_size(&self) -> Option<(u32, u32)> {
        match self {
            Canvas::Framebuffer(fb) => Some(fb.screen_size()),
            Canvas::Image(_path) => None,
        }
    }

    fn draw(&mut self, bitmap: Pixmap) -> Result<(), anyhow::Error> {
        debug!("Drawing bitmap with {}x{} pixels", bitmap.width(), bitmap.height());

        match self {
            Canvas::Framebuffer(fb) => fb.draw(bitmap)?,
            Canvas::Image(path) => bitmap.save_png(path)?,
        };
        Ok(())
    }
}

#[derive(Debug)]
struct WaitForNetwork {
    host: String,
    timeout: Duration,
}

impl WaitForNetwork {
    const INTERVAL: Duration = Duration::from_secs(3);

    fn wait_for_network(&self) -> Result<(), anyhow::Error> {
        debug!("Waiting for network with {}", self.host);

        let start = Instant::now();
        while start.elapsed() < self.timeout {
            match ureq::get(&self.host).call() {
                Ok(_) => return Ok(()),
                Err(e) => debug!("Network probe failed: {}", e),
            };
            thread::sleep(Self::INTERVAL)
        }

        Err(format_err!(
            "Timed out waiting for network: Unable to reach host {:?} after {:?}",
            self.host,
            self.timeout
        ))
    }
}

fn sleeper_from_opts(sleep: opts::Sleep) -> Result<Sleeper, anyhow::Error> {
    let ticker = match sleep.suspend {
        true => Timer::realtime_alarm(sleep.wakeup_rtc)?,
        false => Timer::monotonic()?,
    };

    let mut sleeper = Sleeper::new(sleep.duration, ticker);
    if sleep.suspend {
        sleeper.suspend(true);
    }
    if let Some(e) = sleep.exit_on_keypress {
        let key_devices = KeyDeviceBuilder::with_keys(e.keys)
            .find(&e.devices)
            .context("Failed to access input devices")?;
        sleeper.wakeup_keys(key_devices);
    }

    Ok(sleeper)
}

fn main() -> Result<(), anyhow::Error> {
    dotenvy::dotenv().ok();
    env_logger::init();
    let opts = opts::parse();

    // Template options
    debug!("Loading document: {:?}", &opts.template);
    let template = fs::read(&opts.template)?;

    // Output options
    let mut canvas = Canvas::from_opts(opts.output)?;

    // Script options
    let script = opts.script.map(Script::new);

    // Template and rendering options
    let base_dir = opts.template.canonicalize()?.parent().map(|p| p.to_path_buf());
    let renderer = Renderer::from_config(rendering::Configuration {
        base_dir: base_dir,
        resources_dir: opts.resources_dir,
        fonts_dir: opts.fonts_dir,
        system_fonts: opts.system_fonts,
        screen_size: canvas.screen_size(),
    });

    // Sleep options
    let sleeper = match opts.sleep {
        Some(sleep) => Some(sleeper_from_opts(sleep)?),
        None => None,
    };

    // Network options
    let wait_for_network = opts.wait_for_network.map(|w| WaitForNetwork {
        host: w.host,
        timeout: w.timeout,
    });

    loop {
        // Parse document template
        let mut doc = Document::from_bytes(&template)
            .with_context(|| format!("Failed to load template {:?}", opts.template.to_string_lossy()))?;

        // Wait for network before running script
        if let Some(w) = &wait_for_network {
            w.wait_for_network()?;
        }

        // Manipulate document tree with user script
        if let Some(script) = &script {
            doc = script
                .run_with_document(doc)
                .map_err(|e| format_err!("Failed to execute script: {}", e))?;
        }

        // Render and draw document
        let bitmap = renderer.render(doc).context("Failed to render document")?;
        canvas.draw(bitmap)?;

        // Sleep or exit
        if let Some(sleeper) = &sleeper {
            debug!("Sleeping for {:?}", sleeper.duration());
            let wakeup_reason = sleeper.wait().context("Failed to sleep")?;
            if let WakeupReason::ExitKeyPressed(code) = wakeup_reason {
                debug!("Key {} pressed. Exiting", code);
                break;
            };
        } else {
            break;
        }
    }

    Ok(())
}
