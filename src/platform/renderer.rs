use crate::ui::runtime::PlatformError;

pub(crate) fn choose(app: &crate::AppWindow, mode: &str, save: impl FnOnce() -> bool) {
    let renderer = app.global::<crate::RendererSettings>();
    if !matches!(mode, "cpu" | "gpu") || (mode == "gpu" && !renderer.get_gpu_supported()) {
        return;
    }
    let previous = renderer.get_preferred();
    if previous == mode {
        return;
    }
    renderer.set_preferred(mode.into());
    if !save() {
        renderer.set_preferred(previous);
        renderer.set_error("Could not save the renderer preference. Please try again.".into());
        return;
    }
    renderer.set_error("".into());
    renderer.set_prompt_open(renderer.get_restart_required());
}

pub(crate) fn launch_restart() -> std::io::Result<std::process::Child> {
    // An AppImage's mount disappears with this process, so restart the image file itself.
    let executable = match crate::platform::paths::appimage() {
        Some(image) => image,
        None => std::env::current_exe()?,
    };
    restart_command(executable).spawn()
}

fn restart_command(executable: std::path::PathBuf) -> std::process::Command {
    let mut command = std::process::Command::new(executable);
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW: no console during GUI restart.
    }
    command
}

pub(crate) const GPU_SUPPORTED: bool = cfg!(all(
    feature = "gpu-renderer",
    not(any(target_os = "android", target_os = "ios"))
));

pub(crate) fn select_backend(preferred: &str) -> Result<(&'static str, String), PlatformError> {
    Ok((
        if preferred == "gpu" && GPU_SUPPORTED {
            "gpu"
        } else {
            "cpu"
        },
        String::new(),
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn restart_launches_the_same_executable_in_a_fresh_process() {
        let executable = std::env::current_exe().unwrap();
        let mut command = super::restart_command(executable.clone());
        assert_eq!(command.get_program(), executable.as_os_str());
        assert_eq!(command.get_args().count(), 0);
        // Exercise process creation using the test binary without opening a GUI.
        let mut child = command
            .args([
                "--exact",
                "app::palette::tests::selected_schemes_and_older_settings_resolve_to_supported_options",
            ])
            .spawn()
            .unwrap();
        assert_ne!(child.id(), std::process::id());
        assert!(child.wait().unwrap().success());
    }
}
