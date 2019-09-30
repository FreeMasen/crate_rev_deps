use serde::{Deserialize, Serialize};
use semver::Version;
static BASE: &str = "https://crates.io";
pub const OWNER: usize = 11679;

#[derive(Debug, Deserialize, Serialize)]
pub struct CratesResponse {
    pub crates: Vec<Crate>,
    pub meta: Meta,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Crate {
    pub id: String,
    pub name: String,
    pub downloads: usize,
    pub recent_downloads: usize,
    pub max_version: Version,
    pub description: String,
    pub links: Links,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Links {
    pub version_downloads: String,
    pub versions: String,
    pub owners: String,
    pub owner_team: String,
    pub owner_user: String,
    pub reverse_dependencies: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct ReverseDepsResponse {
    pub versions: Vec<ReverseDep>,
    pub meta: Meta
}
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct ReverseDep {
    pub id: usize,
    #[serde(alias = "crate")]
    pub name: String,
    #[serde(alias = "num")]
    pub version: Version,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Meta {
    pub total: usize,
    pub next_page: Option<String>,
    pub prev_page: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
pub struct CrateInfo {
    pub name: String,
    pub current_version: Version,
    pub downloads: usize,
    pub recent_downloads: usize,
    pub change_in_downloads: Option<usize>,
    pub change_in_recent: Option<isize>,
    pub reverse_deps: Vec<ReverseDep>,
    pub last_checked: chrono::DateTime<chrono::Utc>,
}

pub fn get_crates(user_id: usize) -> Result<Vec<(CrateInfo, String)>, Box<dyn ::std::error::Error>> {
    let raw = get_crates_(user_id)?;
    Ok(raw.crates.into_iter().map(|c| {
        (CrateInfo {
            name: c.name,
            current_version: c.max_version,
            downloads: c.downloads,
            recent_downloads: c.recent_downloads,
            change_in_downloads: None,
            change_in_recent: None,
            reverse_deps: Vec::new(),
            last_checked: chrono::Utc::now(),
        }, c.links.reverse_dependencies)
    }).collect())
}

fn get_crates_(user_id: usize) -> Result<CratesResponse, Box<dyn ::std::error::Error>> {
    let ret = reqwest::get(&format!("{}/api/v1/crates?user_id={}", BASE, user_id))?
        .json()?;
    Ok(ret)
}

pub fn get_rev_deps(endpoint: &str) -> Result<ReverseDepsResponse, Box<dyn ::std::error::Error>> {
    match reqwest::get(&format!("{}{}", BASE, endpoint)) {
        Ok(mut res) => {
            if res.status() != 200 {
                eprintln!("Error getting information");
                let body = res.text()?;
                eprintln!("{}", body);
                Err(Box::new(::std::io::Error::from(::std::io::ErrorKind::Other)))
            } else {
                let ret = res.json()?;
                Ok(ret)
            }
        }
        Err(e) => {
            eprintln!("errror from request\n{}", e);
            Err(Box::new(e))
        }
    }
}

#[derive(Default, Serialize, Debug)]
pub struct CrateChanges {
    name: String,
    new_crate: bool,
    large_change_in_dl: bool,
    new_rev_dep: bool,
}

impl CrateChanges {
    pub fn any_changes(&self) -> bool {
        self.new_crate || self.large_change_in_dl || self.new_rev_dep
    }
}
use std::collections::HashMap;

pub fn perform_analysis(server: &[CrateInfo], files: &[CrateInfo]) -> HashMap<String, CrateChanges> {
    let mut map = HashMap::new();
    let mut ret = HashMap::new();
    for entry in files {
        map.insert(entry.name.to_string(), entry);
    }
    for c in server {
        let mut change = CrateChanges::default();
        if let Some(k) = map.get(&c.name) {
            change.new_rev_dep =  c.reverse_deps != k.reverse_deps;
            change.large_change_in_dl = k.change_in_downloads.unwrap_or(0) > 100;
        } else {
            change.new_crate = true;
            change.new_rev_dep = c.reverse_deps.len() > 0;
        }
        ret.insert(c.name.to_string(), change);
    }
    ret
}
#[cfg(feature = "email")]
pub fn send_message(msg: &str) -> Result<(), Box<dyn ::std::error::Error>> {
use lettre::{Transport, SmtpClient};
use lettre_email::EmailBuilder;
    let email = EmailBuilder::new()
        .to("r.f.masen@gmail.com")
        .subject("Crate Change Report")
        .text(msg)
        .build()?;
    let mut mailer = SmtpClient::new_unencrypted_localhost()?.transport();
    mailer.send(email.into())?;
    Ok(())
}

#[cfg(not(feature = "email"))]
pub fn send_message(msg: &str) -> Result<(), Box<dyn ::std::error::Error>> {
    use lettre::{Transport, FileTransport};
    use lettre_email::EmailBuilder;
    let email = EmailBuilder::new()
        .to("r.f.masen@gmail.com")
        .subject("Crate Change Report")
        .text(msg)
        .build()?;
    let base = if let Some(home) = dirs::home_dir() {
        home
    } else {
        ::std::path::PathBuf::from("")
    };

    let mut mailer = FileTransport::new(base.join(&format!("crate_change_message{}.msg", chrono::Utc::now())));
    mailer.send(email.into())?;
    Ok(())
}