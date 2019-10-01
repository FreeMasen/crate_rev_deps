use serde::{Deserialize, Serialize};
use semver::Version;


static BASE: &str = "https://crates.io";
static HTML: &str = include_str!("template.html");

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
    pub last_checked: chrono::DateTime<chrono::Utc>,
    pub reverse_deps: Vec<ReverseDep>,
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
                let body = res.text()?;
                let parsed = serde_json::from_str(&body);
                if !parsed.is_ok() {
                    eprintln!("Error parsing json");
                    eprintln!("{}", body);
                }
                let ret = parsed?;
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

pub fn perform_analysis(server: &[CrateInfo], files: &[CrateInfo]) -> Vec<ReportCrate> {
    let mut map = HashMap::new();
    let mut ret = Vec::new();
    for entry in files {
        map.insert(entry.name.to_string(), entry);
    }
    for c in server {
        let mut change = CrateChanges::default();
        let old_info = if let Some(k) = map.get(&c.name) {
            change.new_rev_dep =  c.reverse_deps != k.reverse_deps;
            change.large_change_in_dl = k.change_in_downloads.unwrap_or(0) > 100;
            (*k).clone()
        } else {
            change.new_crate = true;
            change.new_rev_dep = c.reverse_deps.len() > 0;
            c.clone()
        };
        ret.push(ReportCrate {
            info: c.clone(),
            old_info,
            changes: change,
        });
    }
    ret
}
#[cfg(feature = "email")]
pub fn send_message(msg: &str) -> Result<(), Box<dyn ::std::error::Error>> {
use lettre::{Transport, SmtpClient};
use lettre_email::EmailBuilder;
    let email = EmailBuilder::new()
        .to("r.f.masen@gmail.com")
        .from("r@robertmasen.pizza")
        .subject("Crate Change Report")
        .html(msg)
        .build()?;
    let mut mailer = SmtpClient::new_unencrypted_localhost()?.transport();
    mailer.send(email.into())?;
    Ok(())
}

#[cfg(not(feature = "email"))]
pub fn send_message(msg: &str) -> Result<(), Box<dyn ::std::error::Error>> {
    use lettre::{Transport, SendableEmail, file::error::FileResult, Envelope};
    use lettre_email::EmailBuilder;
    struct ReadableFileTransport {
        path: ::std::path::PathBuf,
    }
    #[derive(Serialize)]
    struct ReadableEmail {
        envelope: Envelope,
        message_id: String,
        message: String
    }
    impl ReadableFileTransport {
    /// Creates a new transport to the given directory
    pub fn new<P: AsRef<::std::path::Path>>(path: P) -> ReadableFileTransport {
        ReadableFileTransport {
            path: ::std::path::PathBuf::from(path.as_ref()),
        }
    }
}
    impl<'a> Transport<'a> for ReadableFileTransport {
        type Result = FileResult;

        fn send(&mut self, email: SendableEmail) -> FileResult {
            let message_id = email.message_id().to_string();
            let envelope = email.envelope().clone();

            let file = self.path.join(&format!("crate_change_message{}.json", chrono::Utc::now().format("%Y%m%d%H%M%S%Z")));

            let serialized = serde_json::to_string(&ReadableEmail {
                envelope,
                message_id,
                message: email.message_to_string()?,
            })?;

            ::std::fs::write(&file, serialized)?;
            Ok(())
        }
    }
    let email = EmailBuilder::new()
        .to("r.f.masen@gmail.com")
        .subject("Crate Change Report")
        .from("rfm@wiredforge.com")
        .html(msg)
        .build()?;
    let base = if let Some(home) = dirs::home_dir() {
        home
    } else {
        ::std::path::PathBuf::from("")
    };
    // let msg_path = base.join(&format!("crate_change_message{}.msg", chrono::Utc::now().format("%Y%m%d%H%M%S%Z")));
    println!("saving msg to {:?}", base);
    let mut mailer = ReadableFileTransport::new(&base);
    mailer.send(email.into()).map_err(|e| {
        eprintln!("Failed to send");
        e
    })?;
    Ok(())
}
#[derive(Serialize, Debug)]
pub struct ReportCrate {
    pub info: CrateInfo,
    pub old_info: CrateInfo,
    pub changes: CrateChanges,
}

impl ReportCrate {
    pub fn any_changes(&self) -> bool {
        self.changes.any_changes()
    }
}

pub fn generate_html(reports: &[ReportCrate]) -> Result<String, Box<dyn ::std::error::Error>> {
    use tera::{Tera, Context};
    let mut ctx = Context::new();
    ctx.insert("crates", reports);
    let t = Tera::one_off(HTML, &ctx, true)?;
    Ok(t)
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_html() {
        let old = get_old();
        
        let new = get_new();
        
        let reports = perform_analysis(&new, &old);
        
        let html = generate_html(&reports).unwrap();
        ::std::fs::write("test.html", &html).unwrap();
    }
    fn get_old() -> Vec<CrateInfo> {
        vec![
            toml::from_str(&BLOBBER).expect("failed to deserialize blobber"),
            toml::from_str(&MDPP).expect("failed to deserialize mdpp"),
            toml::from_str(&RESS).expect("failed to deserialize ress"),
            toml::from_str(&RESSA).expect("failed to deserialize ressa"),
        ]
    }
    fn get_new() -> Vec<CrateInfo> {
        use std::ops::Add;
        let old = get_old();
        let mut new = Vec::new();
        for (i, info) in old.iter().enumerate() {
            let mut new_info = info.clone();
            if i == 0 {
                new_info.reverse_deps.push(ReverseDep {
                    id: 0,
                    name: "hahahahha".to_string(),
                    version: Version::parse("0.2.0").unwrap(),
                })
            }
            if i == 1 {
                new_info.current_version.minor += 1;
            }
            let new_downloads = (new_info.recent_downloads as f32 * 1.25) as usize;
            new_info.recent_downloads += new_downloads;
            new_info.downloads += new_downloads;
            new_info.change_in_downloads = None;
            new_info.change_in_recent = None;
            let new_checked = new_info.last_checked.add(chrono::Duration::days(1));
            new_info.last_checked = new_checked;
            new.push(new_info);
        }
        new.push(
            toml::from_str(&PROG).expect("failed to deserialize progress_string")
        );
        new
    }
    static BLOBBER: &str = "name = 'blobber'
current_version = '0.1.7'
downloads = 1757
recent_downloads = 302
last_checked = '2019-09-30T22:17:33.917626900Z'
reverse_deps = []";
    static MDPP: &str = "name = 'mdbook-presentation-preprocessor'
current_version = '0.2.2'
downloads = 672
recent_downloads = 344
last_checked = '2019-09-30T22:17:33.917626900Z'
reverse_deps = []";
    static RESS: &str = "name = 'ress'
current_version = '0.8.1'
downloads = 3700
recent_downloads = 1741
last_checked = '2019-09-30T22:17:33.917626900Z'

[[reverse_deps]]
id = 160361
name = 'ressa'
version = '0.6.0'

[[reverse_deps]]
id = 160948
name = 'resw'
version = '0.3.0'
";
    static RESSA: &str = "name = 'ressa'
current_version = '0.7.0-beta-7'
downloads = 2107
recent_downloads = 1232
last_checked = '2019-09-30T22:17:33.917626900Z'

[[reverse_deps]]
id = 156988
name = 'jsyc-compiler'
version = '0.1.0'

[[reverse_deps]]
id = 160948
name = 'resw'
version = '0.3.0'

[[reverse_deps]]
id = 172419
name = 'resast'
version = '0.4.0'
";
    static PROG: &str = "name = 'progress_string'
current_version = '0.1.1'
downloads = 516
recent_downloads = 128
last_checked = '2019-09-30T22:17:33.917626900Z'
reverse_deps = []
";
}