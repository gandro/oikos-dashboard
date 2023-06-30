use std::path::PathBuf;
use std::time::Duration;

use bpaf::{construct, long, positional, Parser};
use humantime;

use crate::evdev::KeyCode;

#[derive(Debug)]
pub enum Output {
    Framebuffer { device: PathBuf, eink_refresh_rate: u32 },
    Image(PathBuf),
}

fn framebuffer() -> impl Parser<Output> {
    let framebuffer = long("framebuffer")
        .env("OIKOS_FRAMEBUFFER")
        .help("Render resulting image into Linux framebuffer")
        .req_flag(());
    let device = long("framebuffer-device")
        .env("OIKOS_FRAMEBUFFER_DEVICE")
        .help("Framebuffer device to be used (default: /dev/fb0)")
        .argument::<PathBuf>("DEVICE")
        .fallback(PathBuf::from("/dev/fb0"));
    let eink_refresh_rate = long("framebuffer-eink-refresh")
        .env("OIKOS_FRAMEBUFFER_EINK_REFRESH")
        .help("Refresh e-ink backed framebuffers every N updates (default: 5)")
        .argument::<u32>("N")
        .fallback(5);

    let output = construct!(Output::Framebuffer {
        device,
        eink_refresh_rate
    });

    construct!(framebuffer, output).map(|((), output)| output)
}

fn image() -> impl Parser<Output> {
    long("image")
        .env("OIKOS_IMAGE")
        .help("Write resulting image to PNG file")
        .argument::<PathBuf>("FILE")
        .map(Output::Image)
}

#[derive(Debug)]
pub struct ExitOnKeypress {
    pub keys: Vec<KeyCode>,
    pub devices: String,
}

#[derive(Debug)]
pub struct Sleep {
    pub duration: Duration,
    pub suspend: bool,
    pub suspend_grace: Duration,
    pub wakeup_rtc: PathBuf,
    pub exit_on_keypress: Option<ExitOnKeypress>,
}

fn sleep() -> impl Parser<Option<Sleep>> {
    let duration = long("sleep")
        .env("OIKOS_SLEEP")
        .help("Sleep and refresh image with this interval")
        .argument::<String>("DURATION")
        .parse(|s| humantime::parse_duration(&s));
    let suspend = long("suspend")
        .env("OIKOS_SUSPEND")
        .help("Suspend to RAM while sleeping")
        .switch();
    let suspend_grace = long("suspend-grace-period")
        .env("OIKOS_SUSPEND_GRACE_PERIOD")
        .help("Wait for this grace period to elapse before suspending to RAM")
        .argument::<String>("DURATION")
        .fallback(String::from("3s"))
        .parse(|s| humantime::parse_duration(&s));
    let wakeup_rtc = long("wakeup-rtc")
        .env("OIKOS_WAKEUP_RTC")
        .help("RTC device to wake-up while suspended (default: /dev/rtc0)")
        .argument::<PathBuf>("DEVICE")
        .fallback(PathBuf::from("/dev/rtc0"));
    let exit_on_keypress_keys = long("exit-on-keypress")
        .env("OIKOS_EXIT_ON_KEYPRESS")
        .help("List of keys which will cause the program to exit when sleeping")
        .argument::<KeyCode>("KEY")
        .some("No valid keys provided");
    let exit_on_keypress_devices = long("exit-on-keypress-devices")
        .env("OIKOS_EXIT_ON_KEYPRESS_DEVICES")
        .help("Input devices to check for exit keypresses")
        .argument::<String>("PATTERN")
        .fallback(String::from("/dev/input/event*"));

    let exit_on_keypress = construct!(ExitOnKeypress{
        keys(exit_on_keypress_keys),
        devices(exit_on_keypress_devices),
    })
    .optional();

    construct!(Sleep {
        duration,
        suspend,
        suspend_grace,
        wakeup_rtc,
        exit_on_keypress,
    })
    .guard(
        |s| s.duration >= Duration::from_secs(30) || s.suspend,
        "Suspend to RAM requires a --sleep duration of at least 30 seconds",
    )
    .group_help("Sleep:")
    .optional()
}

#[derive(Debug)]
pub struct WaitForNetwork {
    pub host: String,
    pub timeout: Duration,
}

fn wait_for_network() -> impl Parser<Option<WaitForNetwork>> {
    let wait_for_network_host = long("wait-for-network")
        .env("OIKOS_WAIT_FOR_NETWORK")
        .help("Wait for connectivity to this HTTP endpoint after standby")
        .argument::<String>("URL");
    let wait_for_network_timeout = long("wait-for-network-timeout")
        .env("OIKOS_WAIT_FOR_NETWORK_TIMEOUT")
        .help("Timeout for network connectivity check")
        .argument::<String>("DURATION")
        .parse(|s| humantime::parse_duration(&s))
        .fallback(Duration::from_secs(30));

    construct!(WaitForNetwork{
        host(wait_for_network_host),
        timeout(wait_for_network_timeout),
    })
    .group_help("Network:")
    .optional()
}

#[derive(Debug)]
pub struct Options {
    // Input template
    pub template: PathBuf,
    // Dynamic scripting
    pub script: Option<PathBuf>,
    pub sleep: Option<Sleep>,
    pub wait_for_network: Option<WaitForNetwork>,
    // Resources for rendering
    pub resources_dir: Option<PathBuf>,
    pub fonts_dir: Option<PathBuf>,
    pub system_fonts: bool,
    // Output canvas
    pub output: Output,
}

fn options() -> impl Parser<Options> {
    let template = positional("TEMPLATE").help("SVG file to be displayed");
    let template_env = long("template")
        .env("OIKOS_TEMPLATE")
        .argument::<PathBuf>("TEMPLATE")
        .hide();
    let template = construct!([template_env, template]);

    let output = construct!([image(), framebuffer()]).group_help("Output:");

    let script = long("script")
        .env("OIKOS_SCRIPT")
        .help("Script used to modify the template before rendering")
        .argument::<PathBuf>("FILE")
        .group_help("Scripting:")
        .optional();

    let resources_dir = long("resources")
        .env("OIKOS_RESOURCES")
        .help("Directory used for resolving relative paths")
        .argument::<PathBuf>("DIR")
        .optional();
    let fonts_dir = long("fonts")
        .env("OIKOS_FONTS")
        .help("Load fonts from this directory")
        .argument::<PathBuf>("DIR")
        .optional();
    let system_fonts = long("system-fonts")
        .env("OIKOS_SYSTEM_FONTS")
        .help("Search for additional fonts in system directories")
        .switch();

    construct!(Options {
        output,
        script,
        sleep(),
        wait_for_network(),
        resources_dir,
        fonts_dir,
        system_fonts,
        // positional argument at the end
        template,
    })
}

pub fn parse() -> Options {
    options().to_options().run()
}
