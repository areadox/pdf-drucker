//! Updates über GitHub Releases: neues Setup laden und still installieren.

use crate::config::{Config, APP_NAME, APP_VERSION, GITHUB_REPO, SETUP_ASSET};
use crate::logic::parse_version;
use anyhow::{anyhow, bail, Context, Result};
use std::os::windows::process::CommandExt;
use std::time::Duration;

pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub size: Option<u64>,
    pub notes: String,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0) // Weiterleitungen selbst folgen, damit der Token nicht an fremde Server geht
        .timeout(Duration::from_secs(60))
        .user_agent(&format!("{APP_NAME}/{APP_VERSION}"))
        .build()
}

fn get(agent: &ureq::Agent, url: &str, token: &str, accept: &str) -> Result<ureq::Response> {
    let mut req = agent.get(url).set("Accept", accept).set("X-GitHub-Api-Version", "2022-11-28");
    if !token.trim().is_empty() {
        req = req.set("Authorization", &format!("Bearer {}", token.trim()));
    }
    match req.call() {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(404, _)) => bail!(
            "Keine Version auf GitHub gefunden.\n\nFalls das Repo privat ist, trage in {} unter \
             \"github_token\" einen Lese-Token ein.",
            crate::config::config_file().display()
        ),
        Err(ureq::Error::Status(401 | 403, _)) => {
            bail!("GitHub hat den Zugriff abgelehnt (Token ungültig oder abgelaufen?).")
        }
        Err(e) => Err(anyhow!("{e}")),
    }
}

pub fn check_update(cfg: &Config) -> Result<Option<UpdateInfo>> {
    let agent = agent();
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let data: serde_json::Value = get(&agent, &url, &cfg.github_token, "application/vnd.github+json")?.into_json()?;

    let tag = data["tag_name"].as_str().unwrap_or_default();
    if parse_version(tag) <= parse_version(APP_VERSION) {
        return Ok(None);
    }
    let asset = data["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(SETUP_ASSET)))
        .with_context(|| format!("Version {tag} enthält kein {SETUP_ASSET}."))?;
    Ok(Some(UpdateInfo {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        url: asset["url"].as_str().unwrap_or_default().to_string(),
        size: asset["size"].as_u64(),
        notes: data["body"].as_str().unwrap_or_default().trim().to_string(),
    }))
}

/// Lädt das Setup und startet es still. Das Setup wartet, bis PDF-Ablage beendet ist,
/// installiert die neue Version und startet sie wieder.
pub fn download_and_run(info: &UpdateInfo, cfg: &Config) -> Result<()> {
    let agent = agent();
    let mut resp = get(&agent, &info.url, &cfg.github_token, "application/octet-stream")?;
    if (300..400).contains(&resp.status()) {
        let location = resp.header("location").context("Download-Weiterleitung ohne Ziel")?.to_string();
        resp = get(&agent, &location, "", "application/octet-stream")?;
    }

    let setup = std::env::temp_dir().join(format!("PDF-Ablage-Setup-{}.exe", info.version));
    {
        let mut file = std::fs::File::create(&setup)?;
        std::io::copy(&mut resp.into_reader(), &mut file)?;
    }
    let data_ok = (|| -> std::io::Result<bool> {
        let size = std::fs::metadata(&setup)?.len();
        let mut head = [0u8; 2];
        std::io::Read::read_exact(&mut std::fs::File::open(&setup)?, &mut head)?;
        Ok(size > 0 && info.size.map_or(true, |s| s == size) && &head == b"MZ")
    })()
    .unwrap_or(false);
    if !data_ok {
        let _ = std::fs::remove_file(&setup);
        bail!("Der Download ist unvollständig oder beschädigt.");
    }

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new(&setup)
        .args(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/UPDATE=1"])
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .context("Setup konnte nicht gestartet werden")?;
    log_info!("Setup für Version {} gestartet.", info.version);
    Ok(())
}
