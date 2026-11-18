mod actors;
mod audio;
mod audio_position;
mod demo;
mod doors;
mod ending;
mod game_palette;
mod highscores;
mod map;
mod opl2;
mod platform;
mod pushwalls;
mod resources;
mod savegame;
mod settings;
mod signon;
mod sprites;
mod symbols;
mod vga;
mod video_filters;
mod vswap;
mod weapons;
mod world;

use std::{env, error::Error, path::PathBuf};

const CONTROL_LABELS: [&str; settings::CONTROL_COUNT] = [
    "Forward",
    "Backward",
    "Turn Left",
    "Turn Right",
    "Run",
    "Strafe Modifier",
    "Use",
    "Fire",
    "Select Knife",
    "Select Pistol",
    "Select Machine Gun",
    "Select Chaingun",
];
const MOVEMENT_DIRECTION_LABELS: [&str; settings::MOVE_DIRECTION_COUNT] =
    ["Move North", "Move East", "Move South", "Move West"];

fn button_action_name(action: u8) -> &'static str {
    match action {
        settings::BUTTON_ACTION_RUN => "Run",
        settings::BUTTON_ACTION_STRAFE => "Strafe",
        settings::BUTTON_ACTION_USE => "Use",
        settings::BUTTON_ACTION_FIRE => "Fire",
        _ => "Unbound",
    }
}

fn assigned_button_name(bindings: &[u8; 4], action: u8, names: &[&str; 4]) -> String {
    bindings
        .iter()
        .position(|&bound| bound == action)
        .map_or_else(|| "Unbound".to_owned(), |button| names[button].to_owned())
}

fn main() {
    let result = run();
    platform::shutdown_sdl();
    if let Err(error) = result {
        eprintln!("Wolfenstein 3D: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let demo = args.iter().any(|arg| arg == "--demo");
    let requested_jukebox = args.iter().any(|arg| arg == "--jukebox");
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    settings::load()?;
    let requested_demo_index = option_value(&args, "--play-demo")?
        .map(|value| value.parse::<usize>())
        .transpose()?;
    let data_dir = option_value_os(&args, "--data")?.map(PathBuf::from);
    let requested_map = option_value(&args, "--map")?
        .map(|value| value.parse::<usize>())
        .transpose()?;
    let requested_difficulty = option_value(&args, "--difficulty")?
        .map(|value| parse_difficulty(&value))
        .transpose()?;
    let requested_save_slot = option_value(&args, "--load-slot")?
        .map(|value| value.parse::<usize>())
        .transpose()?;
    if requested_jukebox
        && (demo
            || requested_demo_index.is_some()
            || requested_map.is_some()
            || requested_difficulty.is_some()
            || requested_save_slot.is_some())
    {
        return Err(
            "--jukebox cannot be combined with game, demo, map, difficulty, or save options".into(),
        );
    }
    if let Some(slot) = requested_save_slot {
        if !(1..=resources::SAVE_SLOT_COUNT).contains(&slot) {
            return Err(format!(
                "--load-slot must be between 1 and {}",
                resources::SAVE_SLOT_COUNT
            )
            .into());
        }
        if requested_map.is_some() {
            return Err("--load-slot cannot be combined with --map".into());
        }
        if demo {
            return Err("--load-slot requires the original game data".into());
        }
    }
    if let Some(index) = requested_demo_index {
        if index >= 4 {
            return Err("--play-demo must be between 0 and 3".into());
        }
        if demo
            || requested_map.is_some()
            || requested_difficulty.is_some()
            || requested_save_slot.is_some()
        {
            return Err(
                "--play-demo cannot be combined with --demo, --map, --difficulty, or --load-slot"
                    .into(),
            );
        }
    }

    if !demo {
        let data = resources::locate(data_dir.as_deref())?;
        let textures = vswap::Vswap::load(&data.directory, data.extension)?;
        let graphics = vga::VgaArchive::load(&data.directory, data.extension)?;
        let pause_sound_pictures = world::PauseSoundMenuPictures::load(&graphics, data.extension)?;
        let menu_background_chunk = if data.extension == "WL1" { 29 } else { 18 };
        let menu_background = graphics
            .picture(menu_background_chunk)?
            .ok_or("VGA menu background picture chunk is unexpectedly sparse")?;
        let main_menu_picture_base = if data.extension == "WL1" { 21 } else { 10 };
        let main_menu_pictures = load_menu_pictures(&graphics, main_menu_picture_base, 3)?;
        let difficulty_picture_base = if data.extension == "WL1" { 30 } else { 19 };
        let difficulty_pictures = load_menu_pictures(&graphics, difficulty_picture_base, 4)?;
        let status_bar_chunk = if data.extension == "WL1" { 90 } else { 86 };
        let status_bar = graphics
            .picture(status_bar_chunk)?
            .ok_or("VGA status bar picture chunk is unexpectedly sparse")?;
        let preload_picture_chunk = if data.extension == "WL1" { 138 } else { 134 };
        let preload_picture = graphics
            .picture(preload_picture_chunk)?
            .ok_or("VGA level preload picture chunk is unexpectedly sparse")?;
        let hud_pictures = world::HudPictures::load(&graphics, data.extension)?;
        let portrait_chunk = if data.extension == "WL1" { 48 } else { 43 };
        let intermission_portrait = graphics
            .picture(portrait_chunk)?
            .ok_or("VGA intermission portrait chunk is unexpectedly sparse")?;
        let intermission_breathing =
            graphics.picture(if data.extension == "WL1" { 88 } else { 84 })?;
        let intermission_portraits = [
            &intermission_portrait,
            intermission_breathing
                .as_ref()
                .unwrap_or(&intermission_portrait),
        ];
        let hero_chunk = if data.extension == "WL1" { 89 } else { 85 };
        let hero_picture = graphics
            .picture(hero_chunk)?
            .ok_or("VGA victory portrait picture chunk is unexpectedly sparse")?;
        let weapon_picture_base = if data.extension == "WL1" { 95 } else { 91 };
        let weapon_pictures = (0..4)
            .map(|index| graphics.picture(weapon_picture_base + index))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or("VGA weapon picture chunk is unexpectedly sparse")?;
        let font = graphics.font(1)?;
        let help_articles = platform::prepare_help_articles(&graphics, data.extension, &font)?;
        let pg13_chunk = if data.extension == "WL1" { 92 } else { 88 };
        let pg13_picture = graphics
            .picture(pg13_chunk)?
            .ok_or("VGA PG-13 picture chunk is unexpectedly sparse")?;
        let title_chunk = if data.extension == "WL1" { 91 } else { 87 };
        let title_picture = graphics
            .picture(title_chunk)?
            .ok_or("VGA title picture chunk is unexpectedly sparse")?;
        let credits_chunk = if data.extension == "WL1" { 93 } else { 89 };
        let credits_picture = graphics
            .picture(credits_chunk)?
            .ok_or("VGA credits picture chunk is unexpectedly sparse")?;
        let sound_effects = audio::AudioArchive::load(&data.directory, data.extension, &textures)?;
        if requested_jukebox {
            let (labels, tracks) = jukebox_tracks(&sound_effects, data.extension);
            if labels.is_empty() {
                return Err("this data set contains no playable jukebox music tracks".into());
            }
            let _ = platform::run_jukebox_menu(
                &labels,
                &tracks,
                &menu_background,
                &font,
                &sound_effects,
            )?;
            return Ok(());
        }
        let mut next_demo = 0usize;
        if requested_demo_index.is_none()
            && requested_map.is_none()
            && requested_save_slot.is_none()
            && requested_difficulty.is_none()
        {
            if !platform::run_signon_screen(&font)? {
                return Ok(());
            }
            let (continue_startup, intro_music) = platform::run_pg13_screen(
                &pg13_picture,
                platform::IntroMusic::new(sound_effects.music_player_for_track(7)),
            )?;
            if !continue_startup {
                return Ok(());
            }
            if !run_attract_cycle(
                &data.directory,
                data.extension,
                &textures,
                &graphics,
                &title_picture,
                &credits_picture,
                &font,
                &sound_effects,
                &status_bar,
                &preload_picture,
                &hud_pictures,
                &pause_sound_pictures,
                &weapon_pictures,
                &mut next_demo,
                intro_music,
            )? {
                return Ok(());
            }
        }
        let available_maps = map::map_count(&data.directory, data.extension)?;
        if let Some(demo_index) = requested_demo_index {
            let recording = load_attract_demo_recording(
                demo_index,
                &data.directory,
                data.extension,
                &graphics,
            )?
            .ok_or_else(|| {
                if data.extension.eq_ignore_ascii_case("WL1") {
                    format!("external demo {demo_index} is unavailable; place DEMO{demo_index}. beside the WL1 files")
                } else {
                    format!("embedded demo {demo_index} is missing from {} data", data.extension)
                }
            })?;
            play_demo_recording(
                recording,
                &data.directory,
                data.extension,
                &textures,
                &graphics,
                &sound_effects,
                &font,
                &status_bar,
                &preload_picture,
                &hud_pictures,
                &pause_sound_pictures,
                &weapon_pictures,
            )?;
            return Ok(());
        }
        let mut selected_save_slot = requested_save_slot;
        let mut menu_music = None;
        if requested_map.is_none() && requested_difficulty.is_none() && selected_save_slot.is_none()
        {
            loop {
                let mut options = vec![
                    "New Game",
                    "Load Game",
                    "High Scores",
                    "Sound",
                    "Display",
                    "Controls",
                    "Back to Demo",
                    "Quit",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
                let show_help = !data.extension.eq_ignore_ascii_case("WL1");
                if show_help {
                    options.insert(6, "Read This!".to_owned());
                }
                let (selection, resumed_music) = platform::run_choice_menu(
                    "Wolfenstein 3D",
                    &options,
                    world::ChoiceMenuLayout::StartupMain,
                    &menu_background,
                    &main_menu_pictures,
                    &font,
                    false,
                    menu_music.or_else(|| sound_effects.music_player_for_track(14)),
                    &sound_effects,
                )?;
                menu_music = resumed_music;
                match selection {
                    platform::ChoiceMenuResult::Selected(0) => break,
                    platform::ChoiceMenuResult::Selected(1) => {
                        let mut slots: Vec<(usize, String)> = (0..resources::SAVE_SLOT_COUNT)
                            .filter_map(|slot| {
                                let path = resources::save_slot_path(slot);
                                savegame::display_name(&path).map(|name| {
                                    let label = if name.trim().is_empty() {
                                        format!("Slot {}", slot + 1)
                                    } else {
                                        format!("Slot {}: {name}", slot + 1)
                                    };
                                    (slot, label)
                                })
                            })
                            .collect();
                        if slots.is_empty() {
                            slots.push((usize::MAX, "Return".to_owned()));
                        }
                        let slot_labels: Vec<_> =
                            slots.iter().map(|(_, label)| label.clone()).collect();
                        let (slot_choice, resumed_music) = platform::run_choice_menu(
                            "Load Game",
                            &slot_labels,
                            world::ChoiceMenuLayout::Main,
                            &menu_background,
                            &[],
                            &font,
                            false,
                            menu_music,
                            &sound_effects,
                        )?;
                        menu_music = resumed_music;
                        if let platform::ChoiceMenuResult::Selected(index) = slot_choice {
                            if let Some((slot, _)) = slots.get(index) {
                                if *slot != usize::MAX {
                                    selected_save_slot = Some(slot + 1);
                                    break;
                                }
                            }
                        } else if slot_choice == platform::ChoiceMenuResult::Quit {
                            return Ok(());
                        }
                    }
                    platform::ChoiceMenuResult::Selected(2) => {
                        if !platform::run_high_scores_menu(
                            &font,
                            &sound_effects,
                            &graphics,
                            data.extension,
                        )? {
                            return Ok(());
                        }
                        menu_music = sound_effects.music_player_for_track(14);
                    }
                    platform::ChoiceMenuResult::Selected(3) => loop {
                        let options = [
                            "Sound Effects: None".to_owned(),
                            "Sound Effects: PC".to_owned(),
                            "Sound Effects: AdLib/SB".to_owned(),
                            "Digitized Effects: None".to_owned(),
                            "Digitized Effects: On".to_owned(),
                            "Music: None".to_owned(),
                            "Music: AdLib/SB".to_owned(),
                            "Audio Output Device".to_owned(),
                            "Back".to_owned(),
                        ];
                        let (selection, resumed_music) = platform::run_choice_menu(
                            "Sound Settings",
                            &options,
                            world::ChoiceMenuLayout::StartupSound,
                            &menu_background,
                            &pause_sound_pictures.sound_settings_art(),
                            &font,
                            false,
                            menu_music,
                            &sound_effects,
                        )?;
                        menu_music = resumed_music;
                        match selection {
                            platform::ChoiceMenuResult::Selected(0) => {
                                settings::set_sound_effects_mode(settings::SoundEffectsMode::Off)?
                            }
                            platform::ChoiceMenuResult::Selected(1) => {
                                settings::set_sound_effects_mode(settings::SoundEffectsMode::Pc)?
                            }
                            platform::ChoiceMenuResult::Selected(2) => {
                                settings::set_sound_effects_mode(settings::SoundEffectsMode::Adlib)?
                            }
                            platform::ChoiceMenuResult::Selected(3) => {
                                settings::set_digitized_effects_enabled(false)?
                            }
                            platform::ChoiceMenuResult::Selected(4) => {
                                settings::set_digitized_effects_enabled(true)?
                            }
                            platform::ChoiceMenuResult::Selected(5) => {
                                settings::set_music_enabled(false)?;
                            }
                            platform::ChoiceMenuResult::Selected(6) => {
                                let music_was_enabled = settings::music_enabled();
                                settings::set_music_enabled(true)?;
                                if !music_was_enabled && settings::music_enabled() {
                                    menu_music = sound_effects.music_player_for_track(14);
                                }
                            }
                            platform::ChoiceMenuResult::Selected(7) => {
                                let devices = platform::audio_output_devices();
                                let current_device = settings::audio_device();
                                let mut device_choices = vec![None];
                                let mut device_options = vec![format!(
                                    "System Default{}",
                                    if current_device.is_none() {
                                        " (Current)"
                                    } else {
                                        ""
                                    }
                                )];
                                for device in &devices {
                                    device_choices.push(Some(device.clone()));
                                    device_options.push(format!(
                                        "{device}{}",
                                        if current_device.as_deref() == Some(device.as_str()) {
                                            " (Current)"
                                        } else {
                                            ""
                                        }
                                    ));
                                }
                                if let Some(current) = current_device.as_ref().filter(|current| {
                                    !devices.iter().any(|device| device == *current)
                                }) {
                                    device_choices.push(Some(current.clone()));
                                    device_options
                                        .push(format!("{current} (Unavailable; saved choice)"));
                                }
                                device_options.push("Back".to_owned());
                                let (device_choice, resumed_music) = platform::run_choice_menu(
                                    "Audio Output Device",
                                    &device_options,
                                    world::ChoiceMenuLayout::Main,
                                    &menu_background,
                                    &[],
                                    &font,
                                    false,
                                    menu_music,
                                    &sound_effects,
                                )?;
                                menu_music = resumed_music;
                                match device_choice {
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index < device_choices.len() =>
                                    {
                                        settings::set_audio_device(device_choices[index].clone())?;
                                    }
                                    platform::ChoiceMenuResult::Selected(_)
                                    | platform::ChoiceMenuResult::Cancelled => {}
                                    platform::ChoiceMenuResult::Quit => return Ok(()),
                                    platform::ChoiceMenuResult::KeyPressed(_) => {}
                                }
                            }
                            platform::ChoiceMenuResult::Selected(8)
                            | platform::ChoiceMenuResult::Cancelled => break,
                            platform::ChoiceMenuResult::Quit => return Ok(()),
                            platform::ChoiceMenuResult::Selected(_)
                            | platform::ChoiceMenuResult::KeyPressed(_) => {}
                        }
                    },
                    platform::ChoiceMenuResult::Selected(4) => loop {
                        let options = [
                            format!(
                                "Fullscreen: {}",
                                if settings::fullscreen_enabled() {
                                    "On"
                                } else {
                                    "Off"
                                }
                            ),
                            format!(
                                "Window Size: {} × {}",
                                platform::window_size().0,
                                platform::window_size().1
                            ),
                            format!("View Size: {}", settings::view_size()),
                            format!("Image Filter: {}", settings::video_filter().label()),
                            format!("World AA: {}", settings::world_renderer().short_label()),
                            format!(
                                "Resolution: {}",
                                settings::render_resolution().short_label()
                            ),
                            format!(
                                "Swastika Symbols: {}",
                                if settings::show_historical_symbols() {
                                    "Show"
                                } else {
                                    "Hide"
                                }
                            ),
                            "Back".to_owned(),
                        ];
                        let (selection, resumed_music) = platform::run_choice_menu(
                            "Display Settings",
                            &options,
                            world::ChoiceMenuLayout::Main,
                            &menu_background,
                            &[],
                            &font,
                            false,
                            menu_music,
                            &sound_effects,
                        )?;
                        menu_music = resumed_music;
                        match selection {
                            platform::ChoiceMenuResult::Selected(0) => {
                                settings::toggle_fullscreen()?
                            }
                            platform::ChoiceMenuResult::Selected(1) => {
                                let sizes = [
                                    "640 × 480 (2×)".to_owned(),
                                    "960 × 720 (3×)".to_owned(),
                                    "1280 × 960 (4×)".to_owned(),
                                ];
                                let (size_choice, resumed_music) = platform::run_choice_menu(
                                    "Window Size",
                                    &sizes,
                                    world::ChoiceMenuLayout::Main,
                                    &menu_background,
                                    &[],
                                    &font,
                                    false,
                                    menu_music,
                                    &sound_effects,
                                )?;
                                menu_music = resumed_music;
                                match size_choice {
                                    platform::ChoiceMenuResult::Selected(index) if index < 3 => {
                                        settings::set_window_scale(index as u8 + 2)?;
                                    }
                                    platform::ChoiceMenuResult::Selected(_)
                                    | platform::ChoiceMenuResult::Cancelled => {}
                                    platform::ChoiceMenuResult::Quit => return Ok(()),
                                    platform::ChoiceMenuResult::KeyPressed(_) => {}
                                }
                            }
                            platform::ChoiceMenuResult::Selected(2) => {
                                let sizes: Vec<String> = (settings::MIN_VIEW_SIZE
                                    ..=settings::MAX_VIEW_SIZE)
                                    .map(|size| format!("View Size {size}"))
                                    .collect();
                                let (size_choice, resumed_music) = platform::run_choice_menu(
                                    "View Size",
                                    &sizes,
                                    world::ChoiceMenuLayout::Main,
                                    &menu_background,
                                    &[],
                                    &font,
                                    false,
                                    menu_music,
                                    &sound_effects,
                                )?;
                                menu_music = resumed_music;
                                if let platform::ChoiceMenuResult::Selected(index) = size_choice {
                                    if index < sizes.len() {
                                        settings::set_view_size(
                                            settings::MIN_VIEW_SIZE + index as u8,
                                        )?;
                                    }
                                }
                                if matches!(size_choice, platform::ChoiceMenuResult::Quit) {
                                    return Ok(());
                                }
                            }
                            platform::ChoiceMenuResult::Selected(3) => {
                                settings::set_video_filter(settings::video_filter().next())?;
                            }
                            platform::ChoiceMenuResult::Selected(4) => {
                                settings::set_world_renderer(settings::world_renderer().next())?;
                            }
                            platform::ChoiceMenuResult::Selected(5) => {
                                settings::set_render_resolution(
                                    settings::render_resolution().next(),
                                )?;
                            }
                            platform::ChoiceMenuResult::Selected(6) => {
                                settings::set_show_historical_symbols(
                                    !settings::show_historical_symbols(),
                                )?;
                            }
                            platform::ChoiceMenuResult::Selected(7)
                            | platform::ChoiceMenuResult::Cancelled => break,
                            platform::ChoiceMenuResult::Quit => return Ok(()),
                            platform::ChoiceMenuResult::Selected(_)
                            | platform::ChoiceMenuResult::KeyPressed(_) => {}
                        }
                    },
                    platform::ChoiceMenuResult::Selected(5) => loop {
                        let panel_options = vec![
                            format!(
                                "Mouse Enabled: {}",
                                if settings::mouse_input_enabled() {
                                    "On"
                                } else {
                                    "Off"
                                }
                            ),
                            format!(
                                "Controller Enabled: {}",
                                if settings::controller_input_enabled() {
                                    "On"
                                } else {
                                    "Off"
                                }
                            ),
                            "Mouse Sensitivity".to_owned(),
                            "Customize Controls".to_owned(),
                            "Restore Defaults".to_owned(),
                            "Back".to_owned(),
                        ];
                        let (selection, resumed_music) = platform::run_choice_menu(
                            "Controls",
                            &panel_options,
                            world::ChoiceMenuLayout::StartupControlPanel,
                            &menu_background,
                            &pause_sound_pictures.control_panel_art(),
                            &font,
                            false,
                            menu_music,
                            &sound_effects,
                        )?;
                        menu_music = resumed_music;
                        match selection {
                            platform::ChoiceMenuResult::Selected(0) => {
                                settings::set_mouse_input_enabled(!settings::mouse_input_enabled())?;
                            }
                            platform::ChoiceMenuResult::Selected(1) => {
                                settings::set_controller_input_enabled(
                                    !settings::controller_input_enabled(),
                                )?;
                            }
                            platform::ChoiceMenuResult::Selected(2) => {
                                let sensitivity_options = (settings::MIN_MOUSE_SENSITIVITY
                                    ..=settings::MAX_MOUSE_SENSITIVITY)
                                    .map(|value| value.to_string())
                                    .collect::<Vec<_>>();
                                let (sensitivity_choice, resumed_music) =
                                    platform::run_choice_menu(
                                        "Mouse Sensitivity",
                                        &sensitivity_options,
                                        world::ChoiceMenuLayout::MouseSensitivity,
                                        &menu_background,
                                        &pause_sound_pictures.mouse_sensitivity_art(),
                                        &font,
                                        false,
                                        menu_music,
                                        &sound_effects,
                                    )?;
                                menu_music = resumed_music;
                                if let platform::ChoiceMenuResult::Selected(value) =
                                    sensitivity_choice
                                {
                                    if value < sensitivity_options.len() {
                                        settings::set_mouse_sensitivity(value as u8)?;
                                    }
                                } else if sensitivity_choice == platform::ChoiceMenuResult::Quit {
                                    return Ok(());
                                }
                            }
                            platform::ChoiceMenuResult::Selected(3) => loop {
                                let controls = settings::get_controls();
                                let mut options: Vec<String> = CONTROL_LABELS
                                    .iter()
                                    .enumerate()
                                    .map(|(index, label)| {
                                        let keys = controls.scancodes[index];
                                        let primary = platform::scancode_name(keys[0]);
                                        let alternate = if keys[1] == 0 {
                                            String::new()
                                        } else {
                                            format!(" / {}", platform::scancode_name(keys[1]))
                                        };
                                        format!("{label}: {primary}{alternate}")
                                    })
                                    .collect();
                                options.push("Customize Mouse Buttons".to_owned());
                                options.push("Customize Controller Buttons".to_owned());
                                options.push("Customize Movement Keys".to_owned());
                                options.push(format!(
                                    "Mouse Sensitivity: {}",
                                    settings::mouse_sensitivity()
                                ));
                                options.push(format!(
                                    "Mouse Input: {}",
                                    if settings::mouse_input_enabled() {
                                        "On"
                                    } else {
                                        "Off"
                                    }
                                ));
                                options.push(format!(
                                    "Controller Input: {}",
                                    if settings::controller_input_enabled() {
                                        "On"
                                    } else {
                                        "Off"
                                    }
                                ));
                                options.push("Restore Defaults".to_owned());
                                options.push("Back".to_owned());
                                let controls_art = pause_sound_pictures.controls_art();
                                let (selection, resumed_music) = platform::run_choice_menu(
                                    "Control Settings",
                                    &options,
                                    world::ChoiceMenuLayout::StartupControls,
                                    &menu_background,
                                    &controls_art,
                                    &font,
                                    false,
                                    menu_music,
                                    &sound_effects,
                                )?;
                                menu_music = resumed_music;
                                match selection {
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index < settings::CONTROL_COUNT =>
                                    {
                                        let heading = format!(
                                            "Press a key for {} (Esc cancels, F1-F10 reserved)",
                                            CONTROL_LABELS[index]
                                        );
                                        let prompt = ["Press any key to bind".to_owned()];
                                        let (captured, resumed_music) = platform::run_choice_menu(
                                            &heading,
                                            &prompt,
                                            world::ChoiceMenuLayout::Controls,
                                            &menu_background,
                                            &[],
                                            &font,
                                            true,
                                            menu_music,
                                            &sound_effects,
                                        )?;
                                        menu_music = resumed_music;
                                        match captured {
                                            platform::ChoiceMenuResult::KeyPressed(scancode) => {
                                                settings::set_control(index, scancode)?;
                                            }
                                            platform::ChoiceMenuResult::Quit => return Ok(()),
                                            platform::ChoiceMenuResult::Selected(_)
                                            | platform::ChoiceMenuResult::Cancelled => {}
                                        }
                                    }
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index == settings::CONTROL_COUNT
                                            || index == settings::CONTROL_COUNT + 1 =>
                                    {
                                        let mouse = index == settings::CONTROL_COUNT;
                                        let actions: &[u8] = if mouse {
                                            &[
                                                settings::BUTTON_ACTION_FIRE,
                                                settings::BUTTON_ACTION_STRAFE,
                                                settings::BUTTON_ACTION_USE,
                                            ]
                                        } else {
                                            &[
                                                settings::BUTTON_ACTION_FIRE,
                                                settings::BUTTON_ACTION_STRAFE,
                                                settings::BUTTON_ACTION_USE,
                                                settings::BUTTON_ACTION_RUN,
                                            ]
                                        };
                                        let device_names = if mouse {
                                            ["Left", "Middle", "Right", "Button 4"]
                                        } else {
                                            ["A", "LB", "B", "X"]
                                        };
                                        loop {
                                            let bindings = settings::button_bindings();
                                            let device_bindings = if mouse {
                                                &bindings.mouse
                                            } else {
                                                &bindings.controller
                                            };
                                            let mut button_options = actions
                                                .iter()
                                                .map(|&action| {
                                                    format!(
                                                        "{}: {}",
                                                        button_action_name(action),
                                                        assigned_button_name(
                                                            device_bindings,
                                                            action,
                                                            &device_names,
                                                        )
                                                    )
                                                })
                                                .collect::<Vec<_>>();
                                            button_options.push("Back".to_owned());
                                            let (button_selection, resumed_music) =
                                                platform::run_choice_menu(
                                                    if mouse {
                                                        "Mouse Button Settings"
                                                    } else {
                                                        "Controller Button Settings"
                                                    },
                                                    &button_options,
                                                    world::ChoiceMenuLayout::Controls,
                                                    &menu_background,
                                                    &[],
                                                    &font,
                                                    false,
                                                    menu_music,
                                                    &sound_effects,
                                                )?;
                                            menu_music = resumed_music;
                                            match button_selection {
                                                platform::ChoiceMenuResult::Selected(
                                                    action_index,
                                                ) if action_index < actions.len() => {
                                                    let action = actions[action_index];
                                                    let heading = format!(
                                                        "Press a {} button for {} (Esc cancels)",
                                                        if mouse { "mouse" } else { "controller" },
                                                        button_action_name(action)
                                                    );
                                                    let (captured, resumed_music) =
                                                        platform::run_button_capture_menu(
                                                            &heading,
                                                            "Press a button to bind",
                                                            mouse,
                                                            &menu_background,
                                                            &font,
                                                            menu_music,
                                                            &sound_effects,
                                                        )?;
                                                    menu_music = resumed_music;
                                                    match captured {
                                                platform::ButtonCaptureResult::Captured(button) => {
                                                    if mouse {
                                                        settings::set_mouse_button_action(
                                                            action, button,
                                                        )?;
                                                    } else {
                                                        settings::set_controller_button_action(
                                                            action, button,
                                                        )?;
                                                    }
                                                }
                                                platform::ButtonCaptureResult::Quit => {
                                                    return Ok(());
                                                }
                                                platform::ButtonCaptureResult::Cancelled => {}
                                            }
                                                }
                                                platform::ChoiceMenuResult::Selected(_)
                                                | platform::ChoiceMenuResult::Cancelled => break,
                                                platform::ChoiceMenuResult::Quit => return Ok(()),
                                                platform::ChoiceMenuResult::KeyPressed(_) => {}
                                            }
                                        }
                                    }
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index == settings::CONTROL_COUNT + 2 =>
                                    {
                                        loop {
                                            let bindings = settings::get_keyboard_move_bindings();
                                            let mut movement_options = MOVEMENT_DIRECTION_LABELS
                                                .iter()
                                                .enumerate()
                                                .map(|(direction, label)| {
                                                    format!(
                                                        "{label}: {}",
                                                        platform::scancode_name(
                                                            bindings.scancodes[direction]
                                                        )
                                                    )
                                                })
                                                .collect::<Vec<_>>();
                                            movement_options.push("Back".to_owned());
                                            let (movement_selection, resumed_music) =
                                                platform::run_choice_menu(
                                                    "Movement Key Settings",
                                                    &movement_options,
                                                    world::ChoiceMenuLayout::Controls,
                                                    &menu_background,
                                                    &[],
                                                    &font,
                                                    false,
                                                    menu_music,
                                                    &sound_effects,
                                                )?;
                                            menu_music = resumed_music;
                                            match movement_selection {
                                                platform::ChoiceMenuResult::Selected(direction)
                                                    if direction
                                                        < settings::MOVE_DIRECTION_COUNT =>
                                                {
                                                    let heading = format!(
                                                        "Press a key for {} (Esc cancels, F1-F10 reserved)",
                                                        MOVEMENT_DIRECTION_LABELS[direction]
                                                    );
                                                    let prompt =
                                                        ["Press any key to bind".to_owned()];
                                                    let (captured, resumed_music) =
                                                        platform::run_choice_menu(
                                                            &heading,
                                                            &prompt,
                                                            world::ChoiceMenuLayout::Controls,
                                                            &menu_background,
                                                            &[],
                                                            &font,
                                                            true,
                                                            menu_music,
                                                            &sound_effects,
                                                        )?;
                                                    menu_music = resumed_music;
                                                    match captured {
                                                        platform::ChoiceMenuResult::KeyPressed(
                                                            scancode,
                                                        ) => {
                                                            settings::set_keyboard_move_key(
                                                                direction, scancode,
                                                            )?;
                                                        }
                                                        platform::ChoiceMenuResult::Quit => {
                                                            return Ok(());
                                                        }
                                                        platform::ChoiceMenuResult::Selected(_)
                                                        | platform::ChoiceMenuResult::Cancelled => {
                                                        }
                                                    }
                                                }
                                                platform::ChoiceMenuResult::Selected(_)
                                                | platform::ChoiceMenuResult::Cancelled => break,
                                                platform::ChoiceMenuResult::Quit => return Ok(()),
                                                platform::ChoiceMenuResult::KeyPressed(_) => {}
                                            }
                                        }
                                    }
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index == settings::CONTROL_COUNT + 3 =>
                                    {
                                        let sensitivity_options = (settings::MIN_MOUSE_SENSITIVITY
                                            ..=settings::MAX_MOUSE_SENSITIVITY)
                                            .map(|value| value.to_string())
                                            .collect::<Vec<_>>();
                                        let sensitivity_art =
                                            pause_sound_pictures.mouse_sensitivity_art();
                                        let (sensitivity_choice, resumed_music) =
                                            platform::run_choice_menu(
                                                "Mouse Sensitivity",
                                                &sensitivity_options,
                                                world::ChoiceMenuLayout::MouseSensitivity,
                                                &menu_background,
                                                &sensitivity_art,
                                                &font,
                                                false,
                                                menu_music,
                                                &sound_effects,
                                            )?;
                                        menu_music = resumed_music;
                                        if let platform::ChoiceMenuResult::Selected(value) =
                                            sensitivity_choice
                                        {
                                            if value < sensitivity_options.len() {
                                                settings::set_mouse_sensitivity(value as u8)?;
                                            }
                                        } else if sensitivity_choice
                                            == platform::ChoiceMenuResult::Quit
                                        {
                                            return Ok(());
                                        }
                                    }
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index == settings::CONTROL_COUNT + 4 =>
                                    {
                                        settings::set_mouse_input_enabled(
                                            !settings::mouse_input_enabled(),
                                        )?;
                                    }
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index == settings::CONTROL_COUNT + 5 =>
                                    {
                                        settings::set_controller_input_enabled(
                                            !settings::controller_input_enabled(),
                                        )?;
                                    }
                                    platform::ChoiceMenuResult::Selected(index)
                                        if index == settings::CONTROL_COUNT + 6 =>
                                    {
                                        settings::reset_controls()?;
                                    }
                                    platform::ChoiceMenuResult::Selected(_)
                                    | platform::ChoiceMenuResult::Cancelled => break,
                                    platform::ChoiceMenuResult::KeyPressed(_)
                                    | platform::ChoiceMenuResult::Quit => return Ok(()),
                                }
                            },
                            platform::ChoiceMenuResult::Selected(4) => {
                                settings::reset_controls()?;
                            }
                            platform::ChoiceMenuResult::Selected(5)
                            | platform::ChoiceMenuResult::Cancelled => break,
                            platform::ChoiceMenuResult::Selected(_) => {}
                            platform::ChoiceMenuResult::KeyPressed(_) => {}
                            platform::ChoiceMenuResult::Quit => return Ok(()),
                        }
                    },
                    platform::ChoiceMenuResult::Selected(6) if show_help => {
                        let _ = platform::run_help_articles(
                            &graphics,
                            data.extension,
                            &font,
                            &sound_effects,
                        )?;
                        menu_music = sound_effects.music_player_for_track(14);
                    }
                    platform::ChoiceMenuResult::Selected(index)
                        if index == (if show_help { 7 } else { 6 }) =>
                    {
                        if !run_attract_cycle(
                            &data.directory,
                            data.extension,
                            &textures,
                            &graphics,
                            &title_picture,
                            &credits_picture,
                            &font,
                            &sound_effects,
                            &status_bar,
                            &preload_picture,
                            &hud_pictures,
                            &pause_sound_pictures,
                            &weapon_pictures,
                            &mut next_demo,
                            platform::IntroMusic::new(sound_effects.music_player_for_track(7)),
                        )? {
                            return Ok(());
                        }
                        menu_music = sound_effects.music_player_for_track(14);
                    }
                    platform::ChoiceMenuResult::Selected(index)
                        if index == (if show_help { 8 } else { 7 }) =>
                    {
                        let (quit, resumed_music) = platform::run_startup_quit_confirmation(
                            &menu_background,
                            &font,
                            menu_music,
                            &sound_effects,
                        )?;
                        menu_music = resumed_music;
                        if quit {
                            return Ok(());
                        }
                    }
                    platform::ChoiceMenuResult::Cancelled => {
                        let (quit, resumed_music) = platform::run_startup_quit_confirmation(
                            &menu_background,
                            &font,
                            menu_music,
                            &sound_effects,
                        )?;
                        menu_music = resumed_music;
                        if quit {
                            return Ok(());
                        }
                    }
                    platform::ChoiceMenuResult::Quit => return Ok(()),
                    platform::ChoiceMenuResult::Selected(_)
                    | platform::ChoiceMenuResult::KeyPressed(_) => {}
                }
            }
        }
        let mut initial_save = if let Some(slot) = selected_save_slot {
            Some(
                platform::load_save_with_progress::<platform::LevelSaveState>(
                    &resources::save_slot_path(slot - 1),
                    &font,
                    &pause_sound_pictures,
                    slot - 1,
                )?,
            )
        } else {
            None
        };
        if let Some(saved) = initial_save.as_ref() {
            if saved.data_variant != data.extension
                || saved.app_version != env!("CARGO_PKG_VERSION")
            {
                return Err(
                    "save slot belongs to a different game data variant or app version".into(),
                );
            }
            if saved.map_number >= available_maps {
                return Err(format!(
                    "save slot refers to map {}, but this data set has only {available_maps} maps",
                    saved.map_number
                )
                .into());
            }
            if requested_difficulty.is_some_and(|difficulty| difficulty != saved.difficulty) {
                return Err("--difficulty does not match the selected save slot".into());
            }
        }
        let mut current_map = if let Some(saved) = initial_save.as_ref() {
            saved.map_number
        } else if let Some(map_number) = requested_map {
            map_number
        } else {
            let episode_count = available_maps.div_ceil(10).min(6);
            if episode_count > 1 {
                let names = [
                    "Episode 1:\nEscape from Wolfenstein",
                    "Episode 2:\nOperation: Eisenfaust",
                    "Episode 3:\nDie, Fuehrer, Die!",
                    "Episode 4:\nA Dark Secret",
                    "Episode 5:\nTrail of the Madman",
                    "Episode 6:\nConfrontation",
                ];
                let options: Vec<_> = names[..episode_count]
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect();
                let episode_picture_base = if data.extension == "WL1" { 41 } else { 30 };
                let mut episode_pictures =
                    load_menu_pictures(&graphics, episode_picture_base, episode_count)?;
                episode_pictures.extend_from_slice(&main_menu_pictures[1..3]);
                let (episode_choice, resumed_menu_music) = platform::run_choice_menu(
                    "Which episode to play?",
                    &options,
                    world::ChoiceMenuLayout::Episode,
                    &menu_background,
                    &episode_pictures,
                    &font,
                    false,
                    sound_effects.music_player_for_track(14),
                    &sound_effects,
                )?;
                menu_music = resumed_menu_music;
                let episode = match episode_choice {
                    platform::ChoiceMenuResult::Selected(episode) => episode,
                    platform::ChoiceMenuResult::Cancelled
                    | platform::ChoiceMenuResult::Quit
                    | platform::ChoiceMenuResult::KeyPressed(_) => {
                        return Ok(());
                    }
                };
                episode * 10
            } else {
                0
            }
        };
        let mut difficulty = if let Some(saved) = initial_save.as_ref() {
            saved.difficulty
        } else if let Some(difficulty) = requested_difficulty {
            difficulty
        } else {
            let options = [
                "Can I play, Daddy?",
                "Don't hurt me.",
                "Bring 'em on!",
                "I am Death incarnate!",
            ]
            .map(str::to_owned);
            let mut difficulty_menu_pictures = difficulty_pictures.clone();
            difficulty_menu_pictures.extend_from_slice(&main_menu_pictures[1..3]);
            let (difficulty_choice, _) = platform::run_choice_menu(
                "How tough are you?",
                &options,
                world::ChoiceMenuLayout::Difficulty,
                &menu_background,
                &difficulty_menu_pictures,
                &font,
                false,
                menu_music
                    .take()
                    .or_else(|| sound_effects.music_player_for_track(14)),
                &sound_effects,
            )?;
            let index = match difficulty_choice {
                platform::ChoiceMenuResult::Selected(index) => index,
                platform::ChoiceMenuResult::Cancelled
                | platform::ChoiceMenuResult::Quit
                | platform::ChoiceMenuResult::KeyPressed(_) => {
                    return Ok(());
                }
            };
            [
                actors::Difficulty::Baby,
                actors::Difficulty::Easy,
                actors::Difficulty::Medium,
                actors::Difficulty::Hard,
            ][index]
        };
        if current_map >= available_maps {
            return Err(format!(
                "map index {current_map} is outside the {available_maps} maps in this data set"
            )
            .into());
        }
        let mut campaign = platform::CampaignState::default();
        let mut last_level_frame = vec![0u32; world::WIDTH * world::HEIGHT];
        'campaign: loop {
            while current_map < available_maps {
                let mut level_map = map::load(&data.directory, data.extension, current_map)?;
                let door_system = doors::DoorSystem::from_map(&level_map).map_err(|message| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
                })?;
                let actors = actors::scan_info_plane(&level_map, difficulty);
                actors::clear_ambush_markers(&mut level_map);
                if let Some(saved) = initial_save.as_ref() {
                    if !saved.is_compatible_with(
                        &level_map,
                        &door_system,
                        difficulty,
                        data.extension,
                    ) {
                        return Err(
                            "save slot does not match the original map data or is invalid".into(),
                        );
                    }
                }
                let sprite_atlas = sprites::SpriteAtlas::load_with_victory(
                    &textures,
                    &actors.actors,
                    level_map.planes[1].contains(&map::EXIT_TILE),
                )?;
                let item_pickups = actors::scan_item_pickups(&level_map);
                match platform::run_level(
                    &level_map,
                    &textures,
                    door_system,
                    item_pickups,
                    &sound_effects,
                    &actors.actors,
                    &sprite_atlas,
                    &font,
                    &status_bar,
                    &preload_picture,
                    &hud_pictures,
                    &pause_sound_pictures,
                    &graphics,
                    &mut last_level_frame,
                    &weapon_pictures,
                    actors.actors.len(),
                    difficulty,
                    data.extension,
                    &mut campaign,
                    initial_save.take(),
                    requested_save_slot.map_or(0, |slot| slot - 1),
                    help_articles.as_ref(),
                    None,
                )? {
                    platform::LevelResult::NewGame {
                        map_number,
                        difficulty: new_difficulty,
                    } => {
                        current_map = map_number;
                        difficulty = new_difficulty;
                        campaign = platform::CampaignState::default();
                        last_level_frame.fill(0);
                        initial_save = None;
                        continue 'campaign;
                    }
                    platform::LevelResult::Complete => {
                        if !platform::run_intermission(
                            current_map,
                            &mut campaign,
                            &hud_pictures,
                            intermission_portraits,
                            &sound_effects,
                            &last_level_frame,
                        )? {
                            return Ok(());
                        }
                        campaign.clear_keys();
                        let Some(next_map) = next_map_index(current_map, available_maps) else {
                            break 'campaign;
                        };
                        current_map = next_map;
                    }
                    platform::LevelResult::Secret => {
                        if !platform::run_intermission(
                            current_map,
                            &mut campaign,
                            &hud_pictures,
                            intermission_portraits,
                            &sound_effects,
                            &last_level_frame,
                        )? {
                            return Ok(());
                        }
                        campaign.clear_keys();
                        let secret_map = (current_map / 10) * 10 + 9;
                        if secret_map >= available_maps {
                            return Err(format!(
                                "this data set has no secret level at map index {secret_map}"
                            )
                            .into());
                        }
                        current_map = secret_map;
                    }
                    platform::LevelResult::Quit => break 'campaign,
                    platform::LevelResult::DemoFinished | platform::LevelResult::DemoAborted => {
                        return Ok(());
                    }
                    platform::LevelResult::GameOver => {
                        let _ = platform::record_campaign_score(
                            &campaign,
                            current_map,
                            &font,
                            &sound_effects,
                            &graphics,
                            data.extension,
                        )?;
                        break 'campaign;
                    }
                    platform::LevelResult::Victory => {
                        let (continue_game, victory_music) = platform::run_victory_screen(
                            &campaign,
                            &hud_pictures,
                            &hero_picture,
                            &sound_effects,
                            &last_level_frame,
                        )?;
                        if !continue_game {
                            return Ok(());
                        }
                        let _ = platform::run_ending_articles(
                            &graphics,
                            data.extension,
                            current_map / 10,
                            &font,
                            victory_music,
                        )?;
                        // Some editions have no ending article; the DOS victory
                        // summary has already been shown above.
                        let _ = platform::record_campaign_score(
                            &campaign,
                            current_map,
                            &font,
                            &sound_effects,
                            &graphics,
                            data.extension,
                        )?;
                        break 'campaign;
                    }
                }
            }
            break 'campaign;
        }
        return Ok(());
    }

    platform::run_demo()
}

fn run_attract_cycle(
    directory: &std::path::Path,
    extension: &str,
    textures: &vswap::Vswap,
    graphics: &vga::VgaArchive,
    title_picture: &vga::Picture,
    credits_picture: &vga::Picture,
    font: &vga::BitmapFont,
    sound_effects: &audio::AudioArchive,
    status_bar: &vga::Picture,
    preload_picture: &vga::Picture,
    hud_pictures: &world::HudPictures,
    pause_sound_pictures: &world::PauseSoundMenuPictures,
    weapon_pictures: &[vga::Picture],
    next_demo: &mut usize,
    mut intro_music: platform::IntroMusic,
) -> Result<bool, Box<dyn Error>> {
    'attract: loop {
        let (title_result, resumed_music) = platform::run_title_screen(title_picture, intro_music)?;
        intro_music = resumed_music;
        match title_result {
            platform::AttractResult::Quit => return Ok(false),
            platform::AttractResult::SkipRemaining => break 'attract,
            platform::AttractResult::TimedOut => {}
        }

        let (credits_result, resumed_music) =
            platform::run_credits_screen(credits_picture, intro_music)?;
        intro_music = resumed_music;
        match credits_result {
            platform::AttractResult::Quit => return Ok(false),
            platform::AttractResult::SkipRemaining => break 'attract,
            platform::AttractResult::TimedOut => {}
        }

        match platform::run_attract_high_scores(
            font,
            sound_effects,
            graphics,
            extension,
            intro_music,
        )? {
            platform::AttractResult::Quit => return Ok(false),
            platform::AttractResult::SkipRemaining => break 'attract,
            platform::AttractResult::TimedOut => {
                let Some(recording) =
                    load_attract_demo_recording(*next_demo, directory, extension, graphics)?
                else {
                    break 'attract;
                };
                let outcome = play_demo_recording(
                    recording,
                    directory,
                    extension,
                    textures,
                    graphics,
                    sound_effects,
                    font,
                    status_bar,
                    preload_picture,
                    hud_pictures,
                    pause_sound_pictures,
                    weapon_pictures,
                )?;
                *next_demo = (*next_demo + 1) % 4;
                match outcome {
                    platform::LevelResult::Quit => return Ok(false),
                    platform::LevelResult::DemoAborted => break 'attract,
                    _ => {
                        intro_music =
                            platform::IntroMusic::new(sound_effects.music_player_for_track(7));
                    }
                }
            }
        }
    }
    Ok(true)
}

fn load_attract_demo_recording(
    demo_index: usize,
    directory: &std::path::Path,
    extension: &str,
    graphics: &vga::VgaArchive,
) -> Result<Option<demo::DemoRecording>, Box<dyn Error>> {
    if extension == "WL1" {
        return load_external_demo(directory, demo_index);
    }
    graphics
        .chunk(139 + demo_index)?
        .map(|bytes| {
            demo::DemoRecording::parse(&bytes)
                .map_err(|error| format!("invalid DOS demo data: {error:?}").into())
        })
        .transpose()
}

fn load_external_demo(
    directory: &std::path::Path,
    demo_index: usize,
) -> Result<Option<demo::DemoRecording>, Box<dyn Error>> {
    for filename in [format!("DEMO{demo_index}."), format!("DEMO{demo_index}")] {
        let path = match resources::find_data_file(directory, &filename) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let bytes = std::fs::read(path)?;
        if bytes.len() > 8_192 {
            return Err(format!("external DOS demo {filename} exceeds 8192 bytes").into());
        }
        return demo::DemoRecording::parse(&bytes)
            .map(Some)
            .map_err(|error| format!("invalid external DOS demo {filename}: {error:?}").into());
    }
    Ok(None)
}

fn play_demo_recording(
    recording: demo::DemoRecording,
    directory: &std::path::Path,
    extension: &str,
    textures: &vswap::Vswap,
    graphics: &vga::VgaArchive,
    audio: &audio::AudioArchive,
    font: &vga::BitmapFont,
    status_bar: &vga::Picture,
    preload_picture: &vga::Picture,
    hud_pictures: &world::HudPictures,
    pause_sound_pictures: &world::PauseSoundMenuPictures,
    weapon_pictures: &[vga::Picture],
) -> Result<platform::LevelResult, Box<dyn Error>> {
    let available_maps = map::map_count(directory, extension)?;
    if recording.map_number >= available_maps {
        return Err(format!(
            "demo refers to map {}, but this data set has only {available_maps} maps",
            recording.map_number
        )
        .into());
    }
    let mut level_map = map::load(directory, extension, recording.map_number)?;
    let door_system = doors::DoorSystem::from_map(&level_map)
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidData, message))?;
    let actors = actors::scan_info_plane(&level_map, actors::Difficulty::Hard);
    actors::clear_ambush_markers(&mut level_map);
    let sprite_atlas = sprites::SpriteAtlas::load(textures, &actors.actors)?;
    let pickups = actors::scan_item_pickups(&level_map);
    let mut campaign = platform::CampaignState::default();
    let mut last_level_frame = vec![0u32; world::WIDTH * world::HEIGHT];
    platform::run_level(
        &level_map,
        textures,
        door_system,
        pickups,
        audio,
        &actors.actors,
        &sprite_atlas,
        font,
        status_bar,
        preload_picture,
        hud_pictures,
        pause_sound_pictures,
        graphics,
        &mut last_level_frame,
        weapon_pictures,
        actors.actors.len(),
        actors::Difficulty::Hard,
        extension,
        &mut campaign,
        None,
        0,
        None,
        Some(recording.playback()),
    )
}

fn load_menu_pictures(
    graphics: &vga::VgaArchive,
    first_chunk: usize,
    count: usize,
) -> Result<Vec<vga::Picture>, Box<dyn Error>> {
    (0..count)
        .map(|index| {
            graphics.picture(first_chunk + index)?.ok_or_else(|| {
                format!(
                    "VGA menu illustration chunk {} is unexpectedly sparse",
                    first_chunk + index
                )
                .into()
            })
        })
        .collect()
}

fn next_map_index(current_map: usize, available_maps: usize) -> Option<usize> {
    let episode = current_map / 10;
    let level = current_map % 10;
    if level == 9 {
        const ELEVATOR_BACK_TO: [usize; 6] = [1, 1, 7, 3, 5, 3];
        let return_level = *ELEVATOR_BACK_TO.get(episode)?;
        let destination = episode * 10 + return_level;
        (destination < available_maps).then_some(destination)
    } else {
        let destination = current_map + 1;
        (destination < available_maps).then_some(destination)
    }
}

fn option_value(
    args: &[std::ffi::OsString],
    option: &str,
) -> Result<Option<String>, Box<dyn Error>> {
    let mut found = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == option {
            if found.is_some() {
                return Err(format!("{option} may only be specified once").into());
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{option} requires a value"))?;
            found = Some(
                value
                    .to_str()
                    .ok_or_else(|| format!("{option} value is not valid Unicode"))?
                    .to_owned(),
            );
            index += 1;
        }
        index += 1;
    }
    Ok(found)
}

fn option_value_os(
    args: &[std::ffi::OsString],
    option: &str,
) -> Result<Option<std::ffi::OsString>, Box<dyn Error>> {
    let mut found = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == option {
            if found.is_some() {
                return Err(format!("{option} may only be specified once").into());
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{option} requires a value"))?;
            found = Some(value.clone());
            index += 1;
        }
        index += 1;
    }
    Ok(found)
}

fn parse_difficulty(value: &str) -> Result<actors::Difficulty, Box<dyn Error>> {
    match value.to_ascii_lowercase().as_str() {
        "baby" => Ok(actors::Difficulty::Baby),
        "easy" => Ok(actors::Difficulty::Easy),
        "medium" => Ok(actors::Difficulty::Medium),
        "hard" => Ok(actors::Difficulty::Hard),
        _ => Err(format!("unknown difficulty '{value}'; use baby, easy, medium, or hard").into()),
    }
}

fn print_help() {
    println!(
        "Wolfenstein 3D Rust port\n\nUsage:\n  wolf3d [--data <directory>] [--map <index>] [--difficulty baby|easy|medium|hard]\n  wolf3d [--data <directory>] --load-slot <1-10>\n  wolf3d [--data <directory>] --play-demo <0-3>\n  wolf3d [--data <directory>] --jukebox\n  wolf3d --demo\n\nThe game requires a complete WL6, WL3, or WL1 data set. Original game data is not included."
    );
}

fn jukebox_tracks(archive: &audio::AudioArchive, extension: &str) -> (Vec<String>, Vec<usize>) {
    let tracks = [
        ("Get Them!", 3),
        ("Searching", 11),
        ("P.O.W.", 9),
        ("Suspense", 12),
        ("War March", 2),
        ("Around The Corner!", 0),
        ("Nazi Anthem", 8),
        ("Lurking...", 18),
        ("Going After Hitler", 17),
        ("Pounding Headache", 4),
        ("Into the Dungeons", 1),
        ("Ultimate Conquest", 19),
        ("Kill the S.O.B.", 6),
        ("The Nazi Rap", 20),
        ("Twelfth Hour", 22),
        ("Zero Hour", 21),
        ("Ultimate Conquest", 19),
        ("Wolfpack", 26),
    ];
    let mut labels = Vec::new();
    let mut indices = Vec::new();
    for (name, track) in tracks {
        if archive.has_music_track(track) {
            let name = match (extension, track) {
                ("WL1", 2) => "Get Out!",
                ("WL1", 26) => "War March",
                _ => name,
            };
            labels.push(name.to_owned());
            indices.push(track);
        }
    }
    (labels, indices)
}

#[cfg(test)]
mod tests {
    use super::next_map_index;

    #[test]
    fn normal_boss_floor_completion_reaches_the_secret_floor() {
        assert_eq!(next_map_index(8, 60), Some(9));
        assert_eq!(next_map_index(18, 60), Some(19));
        assert_eq!(next_map_index(58, 60), Some(59));
        assert_eq!(next_map_index(8, 9), None);
    }

    #[test]
    fn completing_a_secret_floor_returns_to_its_episode_route() {
        assert_eq!(next_map_index(9, 60), Some(1));
        assert_eq!(next_map_index(19, 60), Some(11));
        assert_eq!(next_map_index(59, 60), Some(53));
    }
}
