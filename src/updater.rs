use self_update::cargo_crate_version;

/// GitHub repository owner — update this to your actual GitHub username
const REPO_OWNER: &str = "AymericChaverot";
/// GitHub repository name
const REPO_NAME: &str = "sloth";

/// Checks if a newer release is available on GitHub.
/// Returns `Some(version_string)` if an update exists, `None` otherwise.
pub fn check_for_update() -> Option<String> {
    let current = cargo_crate_version!();

    let latest = match self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("sloth")
        .current_version(current)
        .build()
    {
        Ok(updater) => match updater.get_latest_release() {
            Ok(release) => release.version,
            Err(_) => return None,
        },
        Err(_) => return None,
    };

    let current_ver = semver::Version::parse(current).ok()?;
    let latest_ver = semver::Version::parse(latest.trim_start_matches('v')).ok()?;

    if latest_ver > current_ver {
        Some(latest.clone())
    } else {
        None
    }
}

/// Downloads and installs the latest release, replacing the current binary.
pub fn perform_update() -> Result<(), Box<dyn std::error::Error>> {
    let current = cargo_crate_version!();

    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("sloth")
        .show_download_progress(false)
        .current_version(current)
        .build()?
        .update()?;

    println!("Updated to version: {}", status.version());
    Ok(())
}
